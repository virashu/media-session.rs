use futures::executor::block_on;

use std::cmp::min;
use std::time::{SystemTime, UNIX_EPOCH};

use windows::Foundation::TypedEventHandler;
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as WRT_MediaSession,
    GlobalSystemMediaTransportControlsSessionManager as MediaManager,
    GlobalSystemMediaTransportControlsSessionMediaProperties as MediaProperties,
    GlobalSystemMediaTransportControlsSessionPlaybackInfo as PlaybackInfo,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as PlaybackStatus,
    GlobalSystemMediaTransportControlsSessionTimelineProperties as TimelineProperties,
};

use crate::media_info::MediaInfo;
use crate::playback_state::PlaybackState;
use crate::media_info::MediaInfoInternal;

/// Get UNIX time in microseconds
pub fn micros_since_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_micros() as i64
}

/// Convert Windows NT time to UNIX time
pub fn nt_to_unix(time: i64) -> i64 {
    let microsec_diff = 11_644_473_600_000_000;
    // let sec_diff = 11_644_471_817;
    time - microsec_diff
}

#[derive(Clone, Debug)]
pub struct MediaSession {
    callback: Option<fn(MediaInfo)>,
    manager: MediaManager,
    media_info: MediaInfoInternal,
    session: Option<WRT_MediaSession>,
}

impl MediaSession {
    pub async fn new() -> Self {
        Self {
            callback: None,
            manager: MediaManager::RequestAsync().unwrap().await.unwrap(),
            media_info: MediaInfoInternal::new(),
            session: None,
        }
    }

    pub async fn set_callback(&mut self, callback: fn(MediaInfo)) {
        self.callback = Some(callback);
    }

    pub async fn create_session(&mut self) {
        let session: Result<WRT_MediaSession, _> = self.manager.GetCurrentSession();

        if let Ok(session) = session {
            self.session = Some(session);
            self.setup_listeners();
        }

        self.update().await;
    }

    fn setup_listeners(&mut self) {
        if let Some(session) = &self.session {
            let _ = session.PlaybackInfoChanged(&TypedEventHandler::new({
                let mut player = self.clone();
                move |_, _| {
                    block_on(player.update_playback_info());
                    Ok(())
                }
            }));

            let _ = session.MediaPropertiesChanged(&TypedEventHandler::new({
                let mut player = self.clone();
                move |_, _| {
                    block_on(player.update_media_properties());
                    Ok(())
                }
            }));

            let _ = session.TimelinePropertiesChanged(&TypedEventHandler::new({
                let mut player = self.clone();
                move |_, _| {
                    block_on(player.update_timeline_properties());
                    Ok(())
                }
            }));
        }
    }

    pub fn update_callback(&self) {
        if let Some(callback) = &self.callback {
            callback(self.media_info.info.clone());
        }
    }

    #[allow(dead_code)] // For external use
    pub async fn get_session(&self) -> Option<WRT_MediaSession> {
        self.session.clone()
    }

    pub async fn update(&mut self) {
        if self.session.is_some() {
            self.update_media_properties().await;
            self.update_playback_info().await;
            self.update_timeline_properties().await;
            self.update_position().await;
        }
    }

    async fn update_position(&mut self) {
        match PlaybackState::from_str(self.media_info.info.state.as_ref()) {
            PlaybackState::Stopped => self.media_info.info.position = 0,
            PlaybackState::Paused => self.media_info.info.position = self.media_info.pos_raw,
            PlaybackState::Playing => {
                self.media_info.info.position = self.media_info.pos_raw
                    + (micros_since_epoch() - self.media_info.pos_last_update) // * playback_rate
            }
        }
    }

    #[allow(dead_code)] // For external use
    pub async fn get_info(self) -> MediaInfo {
        let mut info = self.media_info.info.clone();
        let wrapper = self.media_info;

        match PlaybackState::from_str(info.state.as_ref()) {
            PlaybackState::Stopped => info.position = 0,
            PlaybackState::Paused => info.position = wrapper.pos_raw,
            PlaybackState::Playing => {
                let update_delta = micros_since_epoch() - wrapper.pos_last_update;
                let track_delta = update_delta as f64 * info.playback_rate;
                let position = min(info.duration, wrapper.pos_raw + track_delta.round() as i64);
                info.position = position;
            }
        }

        info
    }

    async fn update_playback_info(&mut self) {
        if let Some(session) = &self.session {
            let props: PlaybackInfo = session.GetPlaybackInfo().unwrap();

            self.media_info.info.is_playing = props.PlaybackStatus().unwrap() == PlaybackStatus::Playing;

            self.media_info.info.state = match props.PlaybackStatus().unwrap() {
                PlaybackStatus::Playing => PlaybackState::Playing.into(),
                PlaybackStatus::Paused => PlaybackState::Paused.into(),
                PlaybackStatus::Stopped => PlaybackState::Stopped.into(),

                _ => PlaybackState::Stopped.into(),
            };
            self.media_info.info.playback_rate = props.PlaybackRate().unwrap().Value().unwrap();

            self.update_callback();
        }
    }

    async fn update_media_properties(&mut self) {
        if let Some(session) = &self.session {
            let props: MediaProperties =
                session.TryGetMediaPropertiesAsync().unwrap().await.unwrap();

            self.media_info.info.title = props.Title().unwrap().to_string();
            self.media_info.info.artist = props.Artist().unwrap().to_string();
            self.media_info.info.album_title = props.AlbumTitle().unwrap().to_string();
            self.media_info.info.album_artist = props.AlbumArtist().unwrap().to_string();

            self.update_callback();
        }
    }

    async fn update_timeline_properties(&mut self) {
        if let Some(session) = &self.session {
            let props: TimelineProperties = session.GetTimelineProperties().unwrap();

            // Windows' value is in seconds * 10^-7 (100 nanoseconds)
            // Mapping to micros (10^-6)
            self.media_info.info.duration = props.EndTime().unwrap().Duration / 10;
            self.media_info.pos_raw = props.Position().unwrap().Duration / 10;

            // NT to UNIX in micros
            self.media_info.pos_last_update =
                nt_to_unix(props.LastUpdatedTime().unwrap().UniversalTime / 10);

            self.update_callback();
        }
    }
}
