#![allow(clippy::future_not_send)]

use std::sync::mpsc::{channel, Receiver, Sender};

use base64::{prelude::BASE64_STANDARD, Engine};
use windows::{
    Foundation::{EventRegistrationToken as WRT_EventToken, TypedEventHandler as WRT_EventHandler},
    Media::Control::{
        GlobalSystemMediaTransportControlsSession as WRT_MediaSession,
        GlobalSystemMediaTransportControlsSessionMediaProperties as WRT_MediaProperties,
        GlobalSystemMediaTransportControlsSessionPlaybackInfo as WRT_PlaybackInfo,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus as WRT_PlaybackStatus,
        GlobalSystemMediaTransportControlsSessionTimelineProperties as WRT_TimelineProperties,
    },
};

use crate::{
    imp::windows::utils::stream_ref_to_bytes, utils::nt_to_unix, MediaInfo, PlaybackState,
    PositionInfo,
};

#[allow(clippy::enum_variant_names)]
enum SessionEvent {
    MediaPropertiesChanged,
    PlaybackInfoChanged,
    TimelinePropertiesChanged,
}

#[allow(clippy::struct_field_names)]
struct SessionEventTokens {
    media_properties_changed: WRT_EventToken,
    playback_info_changed: WRT_EventToken,
    timeline_properties_changed: WRT_EventToken,
}

pub struct Session {
    inner: WRT_MediaSession,

    event_channel: (Sender<SessionEvent>, Receiver<SessionEvent>),
    event_tokens: SessionEventTokens,

    media_info: MediaInfo,
    pos_info: PositionInfo,
}

impl Session {
    pub fn new(wrt_session: WRT_MediaSession) -> Self {
        let event_channel = channel();
        let event_tokens = Self::setup_session_events(&wrt_session, &event_channel.0);

        Self {
            inner: wrt_session,
            event_channel,
            event_tokens,
            media_info: MediaInfo::default(),
            pos_info: PositionInfo::default(),
        }
    }

    fn setup_session_events(
        session: &WRT_MediaSession,
        event_sender: &Sender<SessionEvent>,
    ) -> SessionEventTokens {
        let media_properties_changed = session
            .MediaPropertiesChanged(&WRT_EventHandler::new({
                let sender = event_sender.clone();
                move |_, _| {
                    tracing::debug!("Media properties changed");
                    sender.send(SessionEvent::MediaPropertiesChanged).unwrap();
                    Ok(())
                }
            }))
            .unwrap();

        let playback_info_changed = session
            .PlaybackInfoChanged(&WRT_EventHandler::new({
                let sender = event_sender.clone();
                move |_, _| {
                    tracing::debug!("Playback info changed");
                    sender.send(SessionEvent::PlaybackInfoChanged).unwrap();
                    Ok(())
                }
            }))
            .unwrap();

        let timeline_properties_changed = session
            .TimelinePropertiesChanged(&WRT_EventHandler::new({
                let sender = event_sender.clone();
                move |_, _| {
                    tracing::debug!("Timeline properties changed");
                    sender
                        .send(SessionEvent::TimelinePropertiesChanged)
                        .unwrap();
                    Ok(())
                }
            }))
            .unwrap();

        SessionEventTokens {
            media_properties_changed,
            playback_info_changed,
            timeline_properties_changed,
        }
    }

    fn drop_session_events(session: &WRT_MediaSession, tokens: &SessionEventTokens) {
        session
            .RemoveMediaPropertiesChanged(tokens.media_properties_changed)
            .unwrap();
        session
            .RemovePlaybackInfoChanged(tokens.playback_info_changed)
            .unwrap();
        session
            .RemoveTimelinePropertiesChanged(tokens.timeline_properties_changed)
            .unwrap();
    }

    async fn process_events(&mut self) {
        while let Ok(event) = self.event_channel.1.try_recv() {
            _ = match event {
                SessionEvent::MediaPropertiesChanged => self
                    .update_media_properties()
                    .await
                    .inspect_err(|e| tracing::warn!("Failed to update media properties: {e}")),
                SessionEvent::PlaybackInfoChanged => self.update_playback_info(),
                SessionEvent::TimelinePropertiesChanged => self.update_timeline_properties(),
            }
        }
    }

    async fn update_media_properties(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        tracing::debug!("Update: media properties");

        let props: WRT_MediaProperties = self.inner.TryGetMediaPropertiesAsync()?.await?;

        self.media_info.title = props.Title()?.to_string();
        self.media_info.artist = props.Artist()?.to_string();
        self.media_info.album_title = props.AlbumTitle()?.to_string();
        self.media_info.album_artist = props.AlbumArtist()?.to_string();

        match props.Thumbnail() {
            Ok(ref_) => {
                let thumb = stream_ref_to_bytes(ref_).await?;
                self.media_info.cover_raw.clone_from(&thumb);

                let b64 = BASE64_STANDARD.encode(thumb);
                self.media_info.cover_b64 = b64;
            }
            Err(_) => {
                tracing::error!("Failed to get thumbnail");
            }
        }

        Ok(())
    }

    fn update_playback_info(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        tracing::debug!("Update: playback info");

        let props: WRT_PlaybackInfo = self.inner.GetPlaybackInfo()?;

        self.media_info.state = match props.PlaybackStatus()? {
            WRT_PlaybackStatus::Playing => PlaybackState::Playing.into(),
            WRT_PlaybackStatus::Paused => PlaybackState::Paused.into(),
            _ => PlaybackState::Stopped.into(),
        };

        self.pos_info.playback_rate = props.PlaybackRate()?.Value()?;

        Ok(())
    }

    fn update_timeline_properties(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        tracing::debug!("Update: timeline properties");

        let props: WRT_TimelineProperties = self.inner.GetTimelineProperties()?;

        // Windows' value is in seconds * 10^-7 (100 nanoseconds)
        // Mapping to micros (10^-6)
        self.media_info.duration = props.EndTime()?.Duration / 10;
        self.pos_info.pos_raw = props.Position()?.Duration / 10;

        // NT to UNIX in micros
        self.pos_info.pos_last_update = nt_to_unix(props.LastUpdatedTime()?.UniversalTime / 10);

        Ok(())
    }

    pub async fn update_all(&mut self) {
        _ = self.update_media_properties().await;
        _ = self.update_playback_info();
        _ = self.update_timeline_properties();
    }

    pub async fn update(&mut self) {
        self.process_events().await;
    }

    pub fn get_info(&self) -> MediaInfo {
        self.media_info.with_position(&self.pos_info)
    }

    //
    // Controls
    //

    pub async fn pause(&self) -> crate::Result<()> {
        self.inner.TryPauseAsync()?.await?;
        Ok(())
    }

    pub async fn play(&self) -> crate::Result<()> {
        self.inner.TryPlayAsync()?.await?;
        Ok(())
    }

    pub async fn toggle_pause(&self) -> crate::Result<()> {
        self.inner.TryTogglePlayPauseAsync()?.await?;
        Ok(())
    }

    pub async fn stop(&self) -> crate::Result<()> {
        self.inner.TryStopAsync()?.await?;
        Ok(())
    }

    pub async fn next(&self) -> crate::Result<()> {
        self.inner.TrySkipNextAsync()?.await?;
        Ok(())
    }

    pub async fn prev(&self) -> crate::Result<()> {
        self.inner.TrySkipPreviousAsync()?.await?;
        Ok(())
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        Self::drop_session_events(&self.inner, &self.event_tokens);
    }
}
