use std::{cmp::min, fmt::Debug};

use base64::{display::Base64Display, engine::general_purpose::STANDARD};
use windows::{
    Foundation::EventRegistrationToken,
    Media::Control::{
        GlobalSystemMediaTransportControlsSession as WRT_MediaSession,
        GlobalSystemMediaTransportControlsSessionMediaProperties as MediaProperties,
        GlobalSystemMediaTransportControlsSessionPlaybackInfo as PlaybackInfo,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus as PlaybackStatus,
        GlobalSystemMediaTransportControlsSessionTimelineProperties as TimelineProperties,
    },
};

use crate::imp::windows::utils::stream_ref_to_bytes;
use crate::utils::{micros_since_epoch, nt_to_unix};
use crate::{MediaInfo, PlaybackState, PositionInfo};

#[derive(Clone, Debug)]
pub(super) struct EventTokens {
    pub playback_info: EventRegistrationToken,
    pub media_properties: EventRegistrationToken,
    pub timeline_properties: EventRegistrationToken,
}

pub(super) struct Session {
    inner: WRT_MediaSession,
    event_tokens: Option<EventTokens>,

    media_info: MediaInfo,
    pos_info: PositionInfo,
}

impl Session {
    pub fn new(wrt_session: WRT_MediaSession) -> Self {
        let media_info = MediaInfo::default();
        let pos_info = PositionInfo::default();

        Self {
            media_info,
            pos_info,
            inner: wrt_session,
            event_tokens: None,
        }
    }

    pub fn get_session(&self) -> WRT_MediaSession {
        self.inner.clone()
    }

    pub fn set_event_tokens(&mut self, event_tokens: EventTokens) {
        self.event_tokens = Some(event_tokens);
    }

    pub fn clear_event_tokens(&mut self) {
        self.event_tokens = None;
    }

    fn drop_event_listeners(&mut self) {
        if let Some(tokens) = &self.event_tokens {
            self.inner
                .RemoveMediaPropertiesChanged(tokens.media_properties)
                .unwrap();
            self.inner
                .RemovePlaybackInfoChanged(tokens.playback_info)
                .unwrap();
            self.inner
                .RemoveTimelinePropertiesChanged(tokens.timeline_properties)
                .unwrap();
        }
        self.clear_event_tokens();
    }

    #[allow(clippy::future_not_send)]
    pub async fn update_all(&mut self) {
        _ = self
            .update_media_properties()
            .await
            .inspect_err(|_| tracing::warn!("Media properties are not accessible"));
        _ = self
            .update_playback_info()
            .inspect_err(|_| tracing::warn!("Playback info is not accessible"));
        _ = self
            .update_timeline_properties()
            .inspect_err(|_| tracing::warn!("Timeline properties are not accessible"));
    }

    pub fn update_playback_info(&mut self) -> crate::Result<()> {
        tracing::debug!("Updating playback info");

        let props: PlaybackInfo = self.inner.GetPlaybackInfo()?;

        self.media_info.state = match props.PlaybackStatus()? {
            PlaybackStatus::Playing => PlaybackState::Playing.into(),
            PlaybackStatus::Paused => PlaybackState::Paused.into(),
            _ => PlaybackState::Stopped.into(),
        };

        self.pos_info.playback_rate = props.PlaybackRate()?.Value()?;

        Ok(())
    }

    #[allow(clippy::future_not_send)]
    pub async fn update_media_properties(&mut self) -> crate::Result<()> {
        tracing::debug!("Updating media properties");

        let props: MediaProperties = self.inner.TryGetMediaPropertiesAsync()?.await?;

        self.media_info.title = props.Title()?.to_string();
        self.media_info.artist = props.Artist()?.to_string();
        self.media_info.album_title = props.AlbumTitle()?.to_string();
        self.media_info.album_artist = props.AlbumArtist()?.to_string();

        match props.Thumbnail() {
            Ok(ref_) => {
                let thumb = stream_ref_to_bytes(ref_).await?;
                self.media_info.cover_raw.clone_from(&thumb);

                let b64 = Base64Display::new(&thumb, &STANDARD).to_string();
                self.media_info.cover_b64 = b64;
            }
            Err(_) => {
                tracing::error!("Failed to get thumbnail");
            }
        }

        Ok(())
    }

    pub fn update_timeline_properties(&mut self) -> crate::Result<()> {
        tracing::debug!("Updating timeline properties");

        let props: TimelineProperties = self.inner.GetTimelineProperties()?;

        // Windows' value is in seconds * 10^-7 (100 nanoseconds)
        // Mapping to micros (10^-6)
        self.media_info.duration = props.EndTime()?.Duration / 10;
        self.pos_info.pos_raw = props.Position()?.Duration / 10;

        // NT to UNIX in micros
        self.pos_info.pos_last_update = nt_to_unix(props.LastUpdatedTime()?.UniversalTime / 10);

        Ok(())
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
        tracing::debug!("Session dropped");
        self.drop_event_listeners();
    }
}
