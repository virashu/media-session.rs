use std::{
    cmp::min,
    sync::mpsc::{channel, Receiver, Sender},
};

use base64::{prelude::BASE64_STANDARD, Engine};
use windows::{
    Foundation::{EventRegistrationToken as WRT_EventToken, TypedEventHandler as WRT_EventHandler},
    Media::Control::{
        GlobalSystemMediaTransportControlsSession as WRT_MediaSession,
        GlobalSystemMediaTransportControlsSessionManager as WRT_MediaManager,
        GlobalSystemMediaTransportControlsSessionMediaProperties as WRT_MediaProperties,
        GlobalSystemMediaTransportControlsSessionPlaybackInfo as WRT_PlaybackInfo,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus as WRT_PlaybackStatus,
        GlobalSystemMediaTransportControlsSessionTimelineProperties as WRT_TimelineProperties,
    },
};

use crate::{
    imp::windows::utils::stream_ref_to_bytes,
    traits::MediaSessionControls,
    utils::{micros_since_epoch, nt_to_unix},
    MediaInfo, PlaybackState, PositionInfo,
};

enum ManagerEvent {
    CurrentSessionChanged,
}

struct ManagerEventTokens {
    current_session_changed: WRT_EventToken,
}

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

pub struct MediaSession {
    runtime: tokio::runtime::Runtime,

    manager: WRT_MediaManager,
    manager_event_channel: (Sender<ManagerEvent>, Receiver<ManagerEvent>),
    manager_event_tokens: ManagerEventTokens,

    session: Option<WRT_MediaSession>,
    session_event_channel: (Sender<SessionEvent>, Receiver<SessionEvent>),
    session_event_tokens: Option<SessionEventTokens>,

    media_info: MediaInfo,
    pos_info: PositionInfo,
}

impl MediaSession {
    #[allow(clippy::new_without_default, clippy::missing_panics_doc)]
    #[must_use]
    pub fn new() -> Self {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        let manager = runtime
            .block_on(WRT_MediaManager::RequestAsync().unwrap())
            .unwrap();

        let manager_event_channel = channel();
        let manager_event_tokens =
            Self::setup_manager_events(&manager, manager_event_channel.0.clone());

        let session_event_channel = channel();

        let mut self_ = Self {
            runtime,
            manager,
            manager_event_channel,
            manager_event_tokens,
            session: None,
            session_event_channel,
            session_event_tokens: None,
            media_info: MediaInfo::default(),
            pos_info: PositionInfo::default(),
        };

        self_.setup_session();

        self_
    }

    fn setup_manager_events(
        manager: &WRT_MediaManager,
        event_sender: Sender<ManagerEvent>,
    ) -> ManagerEventTokens {
        let token = manager
            .CurrentSessionChanged(&WRT_EventHandler::new(move |_, _| {
                event_sender
                    .send(ManagerEvent::CurrentSessionChanged)
                    .unwrap();
                Ok(())
            }))
            .unwrap();

        ManagerEventTokens {
            current_session_changed: token,
        }
    }

    fn process_manager_events(&mut self) {
        while let Ok(event) = self.manager_event_channel.1.try_recv() {
            match event {
                ManagerEvent::CurrentSessionChanged => self.setup_session(),
            }
        }
    }

    fn setup_session(&mut self) {
        if self.session.is_some() && self.session_event_tokens.is_some() {
            let session = self.session.as_ref().unwrap();
            let tokens = self.session_event_tokens.as_ref().unwrap();
            Self::drop_session_events(session, tokens);

            // Clear
            self.session_event_channel.1.try_iter();

            self.media_info = MediaInfo::default();
            self.pos_info = PositionInfo::default();
        }

        if let Ok(session) = self.manager.GetCurrentSession() {
            let tokens = Self::setup_session_events(&session, &self.session_event_channel.0);
            self.session = Some(session);
            self.session_event_tokens = Some(tokens);
            self.session_update_all();
        } else {
            self.session = None;
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

    fn process_session_events(&mut self) {
        while let Ok(event) = self.session_event_channel.1.try_recv() {
            _ = match event {
                SessionEvent::MediaPropertiesChanged => self.session_update_media_properties(),
                SessionEvent::PlaybackInfoChanged => self.session_update_playback_info(),
                SessionEvent::TimelinePropertiesChanged => {
                    self.session_update_timeline_properties()
                }
            }
        }
    }

    fn session_update_media_properties(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let Some(session) = self.session.as_ref() else {
            return Ok(());
        };

        tracing::debug!("Update: media properties");

        let props: WRT_MediaProperties = self
            .runtime
            .block_on(session.TryGetMediaPropertiesAsync()?)?;

        self.media_info.title = props.Title()?.to_string();
        self.media_info.artist = props.Artist()?.to_string();
        self.media_info.album_title = props.AlbumTitle()?.to_string();
        self.media_info.album_artist = props.AlbumArtist()?.to_string();

        match props.Thumbnail() {
            Ok(ref_) => {
                let thumb = self.runtime.block_on(stream_ref_to_bytes(ref_))?;
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

    fn session_update_playback_info(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let Some(session) = self.session.as_ref() else {
            return Ok(());
        };

        tracing::debug!("Update: playback info");

        let props: WRT_PlaybackInfo = session.GetPlaybackInfo()?;

        self.media_info.state = match props.PlaybackStatus()? {
            WRT_PlaybackStatus::Playing => PlaybackState::Playing.into(),
            WRT_PlaybackStatus::Paused => PlaybackState::Paused.into(),
            _ => PlaybackState::Stopped.into(),
        };

        self.pos_info.playback_rate = props.PlaybackRate()?.Value()?;

        Ok(())
    }

    fn session_update_timeline_properties(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let Some(session) = self.session.as_ref() else {
            return Ok(());
        };

        tracing::debug!("Update: timeline properties");

        let props: WRT_TimelineProperties = session.GetTimelineProperties()?;

        // Windows' value is in seconds * 10^-7 (100 nanoseconds)
        // Mapping to micros (10^-6)
        self.media_info.duration = props.EndTime()?.Duration / 10;
        self.pos_info.pos_raw = props.Position()?.Duration / 10;

        // NT to UNIX in micros
        self.pos_info.pos_last_update = nt_to_unix(props.LastUpdatedTime()?.UniversalTime / 10);

        Ok(())
    }

    fn session_update_all(&mut self) {
        _ = self.session_update_media_properties();
        _ = self.session_update_playback_info();
        _ = self.session_update_timeline_properties();
    }

    pub fn update(&mut self) {
        // Process manager events
        self.process_manager_events();
        // Process session events
        self.process_session_events();
    }

    fn update_position_mut(info: &mut MediaInfo, pos_info: &PositionInfo) {
        let position = match PlaybackState::from(info.state.as_ref()) {
            PlaybackState::Stopped => 0,
            PlaybackState::Paused => pos_info.pos_raw,
            PlaybackState::Playing => {
                let update_delta = micros_since_epoch() - pos_info.pos_last_update;

                #[allow(clippy::cast_precision_loss, reason = "needed for multiplication")]
                let track_delta = update_delta as f64 * pos_info.playback_rate;

                #[allow(clippy::cast_possible_truncation, reason = "rounded")]
                min(info.duration, pos_info.pos_raw + track_delta.round() as i64)
            }
        };

        info.position = position;
    }

    pub fn get_info(&self) -> MediaInfo {
        let mut info = self.media_info.clone();

        Self::update_position_mut(&mut info, &self.pos_info);

        info
    }
}

impl MediaSessionControls for MediaSession {
    fn next(&self) -> crate::Result<()> {
        if let Some(session) = &self.session {
            self.runtime.block_on(session.TrySkipNextAsync()?)?;
        }
        Ok(())
    }
    fn pause(&self) -> crate::Result<()> {
        if let Some(session) = &self.session {
            self.runtime.block_on(session.TryPauseAsync()?)?;
        }
        Ok(())
    }
    fn play(&self) -> crate::Result<()> {
        if let Some(session) = &self.session {
            self.runtime.block_on(session.TryPlayAsync()?)?;
        }
        Ok(())
    }
    fn prev(&self) -> crate::Result<()> {
        if let Some(session) = &self.session {
            self.runtime.block_on(session.TrySkipPreviousAsync()?)?;
        }
        Ok(())
    }
    fn stop(&self) -> crate::Result<()> {
        if let Some(session) = &self.session {
            self.runtime.block_on(session.TryStopAsync()?)?;
        }
        Ok(())
    }
    fn toggle_pause(&self) -> crate::Result<()> {
        if let Some(session) = &self.session {
            self.runtime.block_on(session.TryTogglePlayPauseAsync()?)?;
        }
        Ok(())
    }
}

impl Drop for MediaSession {
    fn drop(&mut self) {
        self.manager
            .RemoveCurrentSessionChanged(self.manager_event_tokens.current_session_changed)
            .unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run() {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .init();

        let mut player = MediaSession::new();
        player.update();

        println!("{:#?}", player.get_info());
    }
}
