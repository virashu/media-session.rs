use futures::executor::block_on;

use windows::Foundation::TypedEventHandler;
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as MediaSession,
    GlobalSystemMediaTransportControlsSessionManager as MediaManager,
    GlobalSystemMediaTransportControlsSessionMediaProperties as MediaProperties,
    GlobalSystemMediaTransportControlsSessionPlaybackInfo as PlaybackInfo,
    GlobalSystemMediaTransportControlsSessionTimelineProperties as TimelineProperties,
};

use crate::media_info::MediaInfo;

#[derive(Clone, Debug)]
pub struct Player {
    manager: MediaManager,
    session: Option<MediaSession>,
    media_info: MediaInfo,
    callback: Option<fn(MediaInfo)>,
}

impl Player {
    pub async fn new(callback: fn(MediaInfo)) -> Player {
        Player {
            manager: MediaManager::RequestAsync().unwrap().await.unwrap(),
            session: None,
            media_info: MediaInfo::new(),
            callback: Some(callback),
        }
    }
    pub async fn create_session(&mut self) {
        let session: Result<MediaSession, _> = self.manager.GetCurrentSession();

        if let Ok(session) = session {
            self.session = Some(session);
            self.setup_listeners();
        }
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
    pub async fn get_session(&self) -> Option<MediaSession> {
        self.session.clone()
    }
    pub async fn update(&mut self) {
        if let Some(session) = &self.session {
            let props: MediaProperties =
                session.TryGetMediaPropertiesAsync().unwrap().await.unwrap();

            self.update_media_properties().await;
        }
    }
    pub async fn get_info(&self) -> MediaInfo {
        self.media_info.clone()
    }
    async fn update_playback_info(&mut self) {
        if let Some(session) = &self.session {
            let props: PlaybackInfo = session.GetPlaybackInfo().unwrap();
        }

        self.update_callback();
    }
    async fn update_media_properties(&mut self) {
        if let Some(session) = &self.session {
            let props: MediaProperties =
                session.TryGetMediaPropertiesAsync().unwrap().await.unwrap();

            self.media_info.title = props.Title().unwrap().to_string();
            self.media_info.artist = props.Artist().unwrap().to_string();
            self.media_info.album_title = props.AlbumTitle().unwrap().to_string();
            self.media_info.album_artist = props.AlbumArtist().unwrap().to_string();
        }

        self.update_callback();
    }
    async fn update_timeline_properties(&mut self) {
        if let Some(session) = &self.session {
            let props: TimelineProperties = session.GetTimelineProperties().unwrap();
        }

        self.update_callback();
    }
}
