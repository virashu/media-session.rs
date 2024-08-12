use std::cmp::min;
use std::fs;
use std::io::Error;
use std::path::Path;

use base64::{display::Base64Display, engine::general_purpose::STANDARD};
use futures::executor::block_on;
use windows::Foundation::TypedEventHandler;
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as WRT_MediaSession,
    GlobalSystemMediaTransportControlsSessionManager as MediaManager,
    GlobalSystemMediaTransportControlsSessionMediaProperties as MediaProperties,
    GlobalSystemMediaTransportControlsSessionPlaybackInfo as PlaybackInfo,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as PlaybackStatus,
    GlobalSystemMediaTransportControlsSessionTimelineProperties as TimelineProperties,
};
use windows::Storage::Streams::{
    Buffer as WRT_Buffer, DataReader as WRT_DataReader,
    IRandomAccessStreamReference as WRT_IStreamRef,
    IRandomAccessStreamWithContentType as WRT_IStream, InputStreamOptions,
};

use crate::utils::{micros_since_epoch, nt_to_unix};
use crate::{MediaInfo, PlaybackState, PositionInfo};

async fn stream_ref_to_bytes(stream_ref: WRT_IStreamRef) -> Vec<u8> {
    let readable_stream: WRT_IStream = stream_ref.OpenReadAsync().unwrap().await.unwrap();
    let read_size = readable_stream.Size().unwrap() as u32;
    let buffer: WRT_Buffer = WRT_Buffer::Create(read_size).unwrap();

    let ib = readable_stream
        .ReadAsync(&buffer, read_size, InputStreamOptions::ReadAhead)
        .unwrap()
        .await
        .unwrap();

    let reader: WRT_DataReader = WRT_DataReader::FromBuffer(&ib).unwrap();
    let len = ib.Length().unwrap() as usize;
    let mut rv: Vec<u8> = vec![0; len];
    let res: &mut [u8] = rv.as_mut_slice();

    reader.ReadBytes(res).unwrap();

    rv
}

#[derive(Clone, Debug)]
pub struct MediaSession {
    callback: Option<fn(MediaInfo)>,
    manager: MediaManager,
    media_info: MediaInfo,
    pos_info: PositionInfo,
    session: Option<WRT_MediaSession>,
}

impl MediaSession {
    pub async fn new() -> Self {
        let mut p = Self {
            callback: None,
            manager: MediaManager::RequestAsync().unwrap().await.unwrap(),
            media_info: MediaInfo::new(),
            pos_info: PositionInfo::new(),
            session: None,
        };

        p.init().await;

        p
    }

    pub async fn init(&mut self) {
        self.create_session().await;
        self.manager
            .SessionsChanged(&TypedEventHandler::new({
                let mut player = self.clone();
                move |_, _| {
                    block_on(player.create_session());
                    Ok(())
                }
            }))
            .unwrap();
    }

    pub fn set_callback(&mut self, callback: fn(MediaInfo)) {
        self.callback = Some(callback);
    }

    pub async fn create_session(&mut self) {
        let session: Result<WRT_MediaSession, _> = self.manager.GetCurrentSession();

        if let Ok(session) = session {
            self.session = Some(session);
            self.setup_session_listeners();
            self.full_update().await;
        }
    }

    fn setup_session_listeners(&mut self) {
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

    pub async fn get_session(&self) -> Option<WRT_MediaSession> {
        self.session.clone()
    }

    pub async fn update(&mut self) {
        if self.session.is_some() {
            self.update_position().await;
        }
    }

    pub async fn full_update(&mut self) {
        self.update_media_properties().await;
        self.update_playback_info().await;
        self.update_timeline_properties().await;

        self.update().await;
    }

    fn update_position_for_mut(info: &mut MediaInfo, pos_info: PositionInfo) {
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

    async fn update_position(&mut self) {
        let info_wrapper = &mut self.media_info;
        Self::update_position_for_mut(info_wrapper, self.pos_info.clone());

        self.update_callback();
    }

    pub async fn get_info(self) -> MediaInfo {
        let mut info_wrapper = self.media_info.clone();

        Self::update_position_for_mut(&mut info_wrapper, self.pos_info.clone());

        info_wrapper.clone()
    }

    async fn update_playback_info(&mut self) {
        if let Some(session) = &self.session {
            let props: PlaybackInfo = session.GetPlaybackInfo().unwrap();

            self.media_info.state = match props.PlaybackStatus().unwrap() {
                PlaybackStatus::Playing => PlaybackState::Playing.into(),
                PlaybackStatus::Paused => PlaybackState::Paused.into(),
                PlaybackStatus::Stopped => PlaybackState::Stopped.into(),

                _ => PlaybackState::Stopped.into(),
            };
            self.pos_info.playback_rate = props.PlaybackRate().unwrap().Value().unwrap();

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

            let ref_ = props.Thumbnail().unwrap();
            let thumb = stream_ref_to_bytes(ref_).await;
            self.media_info.cover_raw = thumb.clone();

            let b64 = Base64Display::new(&thumb, &STANDARD).to_string();
            self.media_info.cover_b64 = b64;

            self.update_callback();
        }
    }

    pub fn write_thumbnail(self, path: &Path) -> Result<(), Error> {
        fs::write(path, self.media_info.cover_raw)
    }

    async fn update_timeline_properties(&mut self) {
        if let Some(session) = &self.session {
            let props: TimelineProperties = session.GetTimelineProperties().unwrap();

            // Windows' value is in seconds * 10^-7 (100 nanoseconds)
            // Mapping to micros (10^-6)
            self.media_info.duration = props.EndTime().unwrap().Duration / 10;
            self.pos_info.pos_raw = props.Position().unwrap().Duration / 10;

            // NT to UNIX in micros
            self.pos_info.pos_last_update =
                nt_to_unix(props.LastUpdatedTime().unwrap().UniversalTime / 10);

            self.update_callback();
        }
    }
}
