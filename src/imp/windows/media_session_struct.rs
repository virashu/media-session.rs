use super::utils::stream_ref_to_bytes;

use std::cmp::min;
use std::fmt::Debug;

use base64::{display::Base64Display, engine::general_purpose::STANDARD};
use log;
use windows::Foundation::EventRegistrationToken;
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as WRT_MediaSession,
    GlobalSystemMediaTransportControlsSessionMediaProperties as MediaProperties,
    GlobalSystemMediaTransportControlsSessionPlaybackInfo as PlaybackInfo,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as PlaybackStatus,
    GlobalSystemMediaTransportControlsSessionTimelineProperties as TimelineProperties,
};

use crate::traits::MediaSessionControls;
use crate::utils::{micros_since_epoch, nt_to_unix};
use crate::{MediaInfo, PlaybackState, PositionInfo};

#[derive(Clone, Debug)]
pub(super) struct EventTokens {
    pub playback_info_changed_token: EventRegistrationToken,
    pub media_properties_changed_token: EventRegistrationToken,
    pub timeline_properties_changed_token: EventRegistrationToken,
}

pub(super) struct MediaSessionStruct {
    media_info: MediaInfo,
    pos_info: PositionInfo,
    session: WRT_MediaSession,
    event_tokens: Option<EventTokens>,
}

impl MediaSessionStruct {
    pub fn new(session: WRT_MediaSession) -> Self {
        let media_info = MediaInfo::new();
        let pos_info = PositionInfo::new();

        let media_session_struct = Self {
            media_info,
            pos_info,
            session,
            event_tokens: None,
        };

        media_session_struct
    }

    pub fn get_session(&self) -> WRT_MediaSession {
        self.session.clone()
    }

    pub fn set_event_tokens(&mut self, event_tokens: EventTokens) {
        self.event_tokens = Some(event_tokens);
    }

    pub fn clear_event_tokens(&mut self) {
        self.event_tokens = None;
    }

    fn drop_event_listeners(&mut self) {
        if let Some(tokens) = &self.event_tokens {
            self.session
                .RemoveMediaPropertiesChanged(tokens.media_properties_changed_token)
                .unwrap();
            self.session
                .RemovePlaybackInfoChanged(tokens.playback_info_changed_token)
                .unwrap();
            self.session
                .RemoveTimelinePropertiesChanged(tokens.timeline_properties_changed_token)
                .unwrap();
        }
        self.clear_event_tokens();
    }

    // async fn update(&mut self) {
    //     self.update_position().await;
    // }

    pub async fn full_update(&mut self) {
        _ = self
            .update_media_properties()
            .await
            .inspect_err(|_| log::warn!("Media properties are not accessible"));
        _ = self
            .update_playback_info()
            .await
            .inspect_err(|_| log::warn!("Playback info is not accessible"));
        _ = self
            .update_timeline_properties()
            .await
            .inspect_err(|_| log::warn!("Timeline properties are not accessible"));
    }

    fn update_position_mut(info: &mut MediaInfo, pos_info: PositionInfo) {
        let position: i64;

        position = match PlaybackState::from_str(info.state.as_ref()) {
            PlaybackState::Stopped => 0,
            PlaybackState::Paused => pos_info.pos_raw,
            PlaybackState::Playing => {
                let update_delta = micros_since_epoch() - pos_info.pos_last_update;
                let track_delta = update_delta as f64 * pos_info.playback_rate;
                min(info.duration, pos_info.pos_raw + track_delta.round() as i64)
            }
        };

        info.position = position;
    }

    // async fn update_position(&mut self) {
    //     let info_wrapper = &mut self.media_info;
    //     Self::update_position_mut(info_wrapper, self.pos_info.clone());
    // }

    pub fn get_info(&self) -> MediaInfo {
        let mut info = self.media_info.clone();

        Self::update_position_mut(&mut info, self.pos_info.clone());

        info
    }

    pub async fn update_playback_info(&mut self) -> crate::Result<()> {
        log::debug!("Updating playback info");

        let props: PlaybackInfo = self.session.GetPlaybackInfo()?;

        self.media_info.state = match props.PlaybackStatus()? {
            PlaybackStatus::Playing => PlaybackState::Playing.into(),
            PlaybackStatus::Paused => PlaybackState::Paused.into(),
            PlaybackStatus::Stopped => PlaybackState::Stopped.into(),

            _ => PlaybackState::Stopped.into(),
        };

        self.pos_info.playback_rate = props.PlaybackRate()?.Value()?;

        Ok(())
    }

    pub async fn update_media_properties(&mut self) -> crate::Result<()> {
        log::debug!("Updating media properties");

        let props: MediaProperties = self.session.TryGetMediaPropertiesAsync()?.await?;

        self.media_info.title = props.Title()?.to_string();
        self.media_info.artist = props.Artist()?.to_string();
        self.media_info.album_title = props.AlbumTitle()?.to_string();
        self.media_info.album_artist = props.AlbumArtist()?.to_string();

        match props.Thumbnail().or(props.Thumbnail()) {
            Ok(ref_) => {
                let thumb = stream_ref_to_bytes(ref_).await?;
                self.media_info.cover_raw = thumb.clone();

                let b64 = Base64Display::new(&thumb, &STANDARD).to_string();
                self.media_info.cover_b64 = b64;
            }
            Err(_) => {
                log::error!("Failed to get thumbnail");
            }
        }

        Ok(())
    }

    pub async fn update_timeline_properties(&mut self) -> crate::Result<()> {
        log::debug!("Updating timeline properties");

        let props: TimelineProperties = self.session.GetTimelineProperties()?;

        // Windows' value is in seconds * 10^-7 (100 nanoseconds)
        // Mapping to micros (10^-6)
        self.media_info.duration = props.EndTime()?.Duration / 10;
        self.pos_info.pos_raw = props.Position()?.Duration / 10;

        // NT to UNIX in micros
        self.pos_info.pos_last_update = nt_to_unix(props.LastUpdatedTime()?.UniversalTime / 10);

        Ok(())
    }
}

impl MediaSessionControls for MediaSessionStruct {
    async fn pause(&self) -> crate::Result<()> {
        self.session.TryPauseAsync()?.await?;

        Ok(())
    }

    async fn play(&self) -> crate::Result<()> {
        self.session.TryPlayAsync()?.await?;

        Ok(())
    }

    async fn toggle_pause(&self) -> crate::Result<()> {
        self.session.TryTogglePlayPauseAsync()?.await?;

        Ok(())
    }

    async fn stop(&self) -> crate::Result<()> {
        self.session.TryStopAsync()?.await?;

        Ok(())
    }

    async fn next(&self) -> crate::Result<()> {
        self.session.TrySkipNextAsync()?.await?;

        Ok(())
    }

    async fn prev(&self) -> crate::Result<()> {
        self.session.TrySkipPreviousAsync()?.await?;

        Ok(())
    }
}

impl Drop for MediaSessionStruct {
    fn drop(&mut self) {
        log::debug!("Session dropped");
        self.drop_event_listeners();
    }
}
