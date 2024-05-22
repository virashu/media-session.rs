use futures::executor::block_on;

use std::time::{SystemTime, UNIX_EPOCH};

use windows::Foundation::TypedEventHandler;
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as MediaSession,
    GlobalSystemMediaTransportControlsSessionManager as MediaManager,
    GlobalSystemMediaTransportControlsSessionMediaProperties as MediaProperties,
    GlobalSystemMediaTransportControlsSessionPlaybackInfo as PlaybackInfo,
    GlobalSystemMediaTransportControlsSessionTimelineProperties as TimelineProperties,
};

use crate::media_info::MediaInfo;

pub fn micros_since_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_micros() as i64
}

#[derive(Clone, Debug)]
pub struct Player {
    callback: Option<fn(MediaInfo)>,
    manager: MediaManager,
    media_info: MediaInfo,
    session: Option<MediaSession>,
}

impl Player {
    pub async fn new(callback: fn(MediaInfo)) -> Player {
        Player {
            callback: Some(callback),
            manager: MediaManager::RequestAsync().unwrap().await.unwrap(),
            media_info: MediaInfo::new(),
            session: None,
        }
    }

    pub async fn create_session(&mut self) {
        let session: Result<MediaSession, _> = self.manager.GetCurrentSession();

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
            callback(self.media_info.clone());
        }
    }
    #[allow(dead_code)] // For external use
    pub async fn get_session(&self) -> Option<MediaSession> {
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
        // get current time (UTC)
        let cur = micros_since_epoch();

        self.media_info.position =
            self.media_info.pos_raw + (cur - self.media_info.pos_last_update);
    }

    #[allow(dead_code)] // For external use
    pub async fn get_info(&mut self) -> MediaInfo {
        self.update_position().await;
        self.media_info.clone()
    }

    async fn update_playback_info(&mut self) {
        if let Some(session) = &self.session {
            #[allow(unused_variables)]
            let props: PlaybackInfo = session.GetPlaybackInfo().unwrap();

            self.update_callback();
        }
    }

    async fn update_media_properties(&mut self) {
        if let Some(session) = &self.session {
            let props: MediaProperties =
                session.TryGetMediaPropertiesAsync().unwrap().await.unwrap();

            self.media_info.title = props.Title().unwrap().to_string();
            self.media_info.artist = props.Artist().unwrap().to_string();
            self.media_info.album_title = props.AlbumTitle().unwrap().to_string();
            self.media_info.album_artist = props.AlbumArtist().unwrap().to_string();

            self.update_callback();
        }
    }

    async fn update_timeline_properties(&mut self) {
        if let Some(session) = &self.session {
            let props: TimelineProperties = session.GetTimelineProperties().unwrap();

            // Windows' value is in seconds * 10^-7
            // Mapping to micros (10^-6)
            self.media_info.duration = props.EndTime().unwrap().Duration / 10;
            self.media_info.pos_raw = props.Position().unwrap().Duration / 10;

            self.media_info.pos_last_update = props.LastUpdatedTime().unwrap().UniversalTime;

            self.update_callback();
        }
    }
}
