use log;
use std::cmp::min;
use std::fmt::Debug;
use std::fs;
use std::path::Path;
use std::sync::mpsc::{channel, Receiver, Sender};

use base64::{display::Base64Display, engine::general_purpose::STANDARD};
use windows::core::Error as WRT_Error;
use windows::Foundation::{EventRegistrationToken, TypedEventHandler};
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
use crate::Error;
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

#[derive(Debug)]
enum MediaSessionEvent {
    PlaybackInfoChanged,
    MediaPropertiesChanged,
    TimelinePropertiesChanged,
}

#[derive(Debug)]
enum MediaManagerEvent {
    SessionChanged,
}

#[derive(Debug)]
struct EventChannel<T> {
    sender: Sender<T>,
    receiver: Receiver<T>,
}

type SessionEventChannel = EventChannel<MediaSessionEvent>;
type ManagerEventChannel = EventChannel<MediaManagerEvent>;

#[derive(Clone, Debug)]
struct EventTokens {
    playback_info_changed_token: EventRegistrationToken,
    media_properties_changed_token: EventRegistrationToken,
    timeline_properties_changed_token: EventRegistrationToken,
}

pub struct MediaSession {
    callback: Option<Box<dyn Fn(MediaInfo)>>,
    manager: MediaManager,
    media_info: MediaInfo,
    pos_info: PositionInfo,
    session: Option<WRT_MediaSession>,
    event_tokens: Option<EventTokens>,
    session_event_channel: SessionEventChannel,
    manager_event_channel: ManagerEventChannel,
}

impl MediaSession {
    pub async fn new() -> Self {
        let manager = MediaManager::RequestAsync().unwrap().await.unwrap();

        let (sender, receiver) = channel::<MediaSessionEvent>();
        let session_event_channel = SessionEventChannel { receiver, sender };

        let (sender, receiver) = channel::<MediaManagerEvent>();
        let manager_event_channel = ManagerEventChannel { receiver, sender };

        let mut p = Self {
            manager,
            media_info: MediaInfo::new(),
            pos_info: PositionInfo::new(),
            session: None,
            event_tokens: None,
            callback: None,
            session_event_channel,
            manager_event_channel,
        };

        p.init().await;

        p
    }

    async fn init(&mut self) {
        self.manager
            .SessionsChanged(&TypedEventHandler::new({
                let s = self.manager_event_channel.sender.clone();
                move |_, _| {
                    s.send(MediaManagerEvent::SessionChanged).unwrap();
                    Ok(())
                }
            }))
            .unwrap();
        self.create_session().await;
    }

    pub fn set_callback<F>(&mut self, callback: F) where F: Fn(MediaInfo) + 'static {
        self.callback = Some(Box::new(callback));
    }

    async fn create_session(&mut self) {
        self.drop_session_listeners();

        let session: Result<WRT_MediaSession, _> = self.manager.GetCurrentSession();

        if let Ok(session) = session {
            self.session = Some(session);
            self.event_tokens = Some(self.setup_session_listeners().unwrap());
            self.full_update().await;
        }
    }

    fn drop_session_listeners(&mut self) {
        if let Some(tokens) = &self.event_tokens {
            if let Some(session) = &self.session {
                session
                    .RemoveMediaPropertiesChanged(tokens.media_properties_changed_token)
                    .unwrap();
                session
                    .RemovePlaybackInfoChanged(tokens.playback_info_changed_token)
                    .unwrap();
                session
                    .RemoveTimelinePropertiesChanged(tokens.timeline_properties_changed_token)
                    .unwrap();
            }

            self.event_tokens = None;
        }
    }

    fn setup_session_listeners(&mut self) -> Result<EventTokens, Box<dyn std::error::Error>> {
        let sender = &self.session_event_channel.sender;

        if let Some(session) = &self.session {
            let playback_info_changed_token = session
                .PlaybackInfoChanged(&TypedEventHandler::new({
                    let s = sender.clone();
                    move |_, _| {
                        s.send(MediaSessionEvent::PlaybackInfoChanged).unwrap();
                        Ok(())
                    }
                }))
                .unwrap();

            let media_properties_changed_token = session
                .MediaPropertiesChanged(&TypedEventHandler::new({
                    let s = sender.clone();
                    move |_, _| {
                        s.send(MediaSessionEvent::MediaPropertiesChanged).unwrap();
                        Ok(())
                    }
                }))
                .unwrap();

            let timeline_properties_changed_token =
                session.TimelinePropertiesChanged(&TypedEventHandler::new({
                    let s = sender.clone();
                    move |_, _| {
                        s.send(MediaSessionEvent::TimelinePropertiesChanged)
                            .unwrap();
                        Ok(())
                    }
                }))?;

            Ok(EventTokens {
                media_properties_changed_token,
                playback_info_changed_token,
                timeline_properties_changed_token,
            })
        } else {
            Err(Box::new(Error::new("No active session")))
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
        if let Ok(event) = self.manager_event_channel.receiver.try_recv() {
            match event {
                MediaManagerEvent::SessionChanged => self.create_session().await,
            }
        }

        if self.session.is_some() {
            while let Ok(event) = self.session_event_channel.receiver.try_recv() {
                log::debug!("Got event: {:?}", event);
                match event {
                    MediaSessionEvent::MediaPropertiesChanged => {
                        self.update_media_properties().await.unwrap()
                    }
                    MediaSessionEvent::PlaybackInfoChanged => {
                        self.update_playback_info().await.unwrap()
                    }
                    MediaSessionEvent::TimelinePropertiesChanged => {
                        self.update_timeline_properties().await.unwrap()
                    }
                }
            }
            self.update_position().await;
            self.update_callback();
        }
    }

    pub async fn full_update(&mut self) {
        self.update_media_properties().await.unwrap();
        self.update_playback_info().await.unwrap();
        self.update_timeline_properties().await.unwrap();
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
    }

    pub fn get_info(self) -> MediaInfo {
        let mut info_wrapper = self.media_info.clone();

        Self::update_position_for_mut(&mut info_wrapper, self.pos_info.clone());

        info_wrapper.clone()
    }

    pub fn get_info_from(ms: &MediaSession) -> MediaInfo {
        let mut info_wrapper = ms.media_info.clone();

        Self::update_position_for_mut(&mut info_wrapper, ms.pos_info.clone());

        info_wrapper.clone()
    }

    async fn update_playback_info(&mut self) -> Result<(), WRT_Error> {
        log::debug!("Updating playback info");

        if let Some(session) = &self.session {
            Self::update_playback_info_mut(&mut self.media_info, &mut self.pos_info, session)
                .await?;
        }
        Ok(())
    }

    async fn update_playback_info_mut(
        media_info: &mut MediaInfo,
        pos_info: &mut PositionInfo,
        session: &WRT_MediaSession,
    ) -> Result<(), WRT_Error> {
        let props: PlaybackInfo = session.GetPlaybackInfo()?;

        media_info.state = match props.PlaybackStatus()? {
            PlaybackStatus::Playing => PlaybackState::Playing.into(),
            PlaybackStatus::Paused => PlaybackState::Paused.into(),
            PlaybackStatus::Stopped => PlaybackState::Stopped.into(),

            _ => PlaybackState::Stopped.into(),
        };
        pos_info.playback_rate = props.PlaybackRate()?.Value()?;

        Ok(())
    }

    async fn update_media_properties(&mut self) -> Result<(), WRT_Error> {
        log::debug!("Updating media properties");

        if let Some(session) = &self.session {
            let props: MediaProperties = session.TryGetMediaPropertiesAsync()?.await?;

            self.media_info.title = props.Title().unwrap().to_string();
            self.media_info.artist = props.Artist().unwrap().to_string();
            self.media_info.album_title = props.AlbumTitle().unwrap().to_string();
            self.media_info.album_artist = props.AlbumArtist().unwrap().to_string();

            let ref_ = props.Thumbnail().unwrap();
            let thumb = stream_ref_to_bytes(ref_).await;
            self.media_info.cover_raw = thumb.clone();

            let b64 = Base64Display::new(&thumb, &STANDARD).to_string();
            self.media_info.cover_b64 = b64;
        }

        Ok(())
    }

    fn write_thumbnail(self, path: &Path) -> Result<(), std::io::Error> {
        fs::write(path, self.media_info.cover_raw.clone())
    }

    async fn update_timeline_properties(&mut self) -> Result<(), WRT_Error> {
        log::debug!("Updating timeline properties");

        if let Some(session) = &self.session {
            let props: TimelineProperties = session.GetTimelineProperties()?;

            // Windows' value is in seconds * 10^-7 (100 nanoseconds)
            // Mapping to micros (10^-6)
            self.media_info.duration = props.EndTime()?.Duration / 10;
            self.pos_info.pos_raw = props.Position()?.Duration / 10;

            // NT to UNIX in micros
            self.pos_info.pos_last_update = nt_to_unix(props.LastUpdatedTime()?.UniversalTime / 10);
        }

        Ok(())
    }
}

pub trait MediaSessionControls {
    async fn toggle_pause(self) -> Result<(), WRT_Error>;
    async fn pause(self) -> Result<(), WRT_Error>;
    async fn play(self) -> Result<(), WRT_Error>;
    async fn stop(self) -> Result<(), WRT_Error>;
    async fn next(self) -> Result<(), WRT_Error>;
    async fn prev(self) -> Result<(), WRT_Error>;
}

impl MediaSessionControls for MediaSession {
    async fn pause(self) -> Result<(), WRT_Error> {
        if let Some(session) = &self.session {
            session.TryPauseAsync()?.await?;
        }

        Ok(())
    }

    async fn play(self) -> Result<(), WRT_Error> {
        if let Some(session) = &self.session {
            session.TryPlayAsync()?.await?;
        }

        Ok(())
    }

    async fn toggle_pause(self) -> Result<(), WRT_Error> {
        if let Some(session) = &self.session {
            session.TryTogglePlayPauseAsync()?.await?;
        }

        Ok(())
    }

    async fn stop(self) -> Result<(), WRT_Error> {
        if let Some(session) = &self.session {
            session.TryStopAsync()?.await?;
        }

        Ok(())
    }

    async fn next(self) -> Result<(), WRT_Error>{
        if let Some(session) = &self.session {
            session.TrySkipNextAsync()?.await?;
        }

        Ok(())
    }

    async fn prev(self) -> Result<(), WRT_Error>{
        if let Some(session) = &self.session {
            session.TrySkipPreviousAsync()?.await?;
        }

        Ok(())
    }
}

impl Drop for MediaSession {
    fn drop(&mut self) {
        self.drop_session_listeners();
    }
}
