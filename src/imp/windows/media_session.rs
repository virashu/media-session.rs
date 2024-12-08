use crate::traits::MediaSessionControls;
use crate::MediaInfo;

use futures::executor::block_on;

use super::media_session_struct::EventTokens;
use super::media_session_struct::MediaSessionStruct;

use std::sync::{Arc, Mutex};

use windows::Foundation::{EventRegistrationToken, TypedEventHandler};

use windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager as WRT_MediaManager;

pub struct MediaSession {
    manager: WRT_MediaManager,
    session: Arc<Mutex<Option<MediaSessionStruct>>>,
    event_token: Option<EventRegistrationToken>,
}

impl MediaSession {
    pub async fn new() -> Self {
        let manager = WRT_MediaManager::RequestAsync().unwrap().await.unwrap();
        let session_opt = Self::create_session(&Some(manager.clone()));
        let session = Arc::new(Mutex::new(session_opt));

        Self::update_session_async(&session).await;

        let mut media_session = Self {
            manager,
            session,
            event_token: None,
        };

        media_session.setup_manager_listeners();

        media_session
    }

    fn setup_manager_listeners(&mut self) {
        let session = Arc::clone(&self.session);

        let token = self
            .manager
            .CurrentSessionChanged(&TypedEventHandler::new(move |manager, _| {
                {
                    let mut s = session.lock().unwrap();
                    *s = Self::create_session(manager);
                }
                Self::setup_session_listeners(&session);
                Self::update_session(&session);

                Ok(())
            }))
            .unwrap();

        self.event_token = Some(token);
    }

    fn setup_session_listeners(session_mutex: &Arc<Mutex<Option<MediaSessionStruct>>>) {
        let mut session_opt = session_mutex.lock().unwrap();

        if let Some(session) = &mut *session_opt {
            let wrt_session = session.get_session();

            let session_clone = Arc::clone(session_mutex);
            let playback_info_changed_token = wrt_session
                .PlaybackInfoChanged(&TypedEventHandler::new(move |_, _| {
                    if let Some(session) = &mut *session_clone.lock().unwrap() {
                        let _ = block_on(session.update_playback_info())
                            .inspect_err(|e| log::warn!("Failed to update playback info: {e}"));
                    }
                    Ok(())
                }))
                .unwrap();

            let session_clone = Arc::clone(session_mutex);
            let media_properties_changed_token = wrt_session
                .MediaPropertiesChanged(&TypedEventHandler::new(move |_, _| {
                    if let Some(session) = &mut *session_clone.lock().unwrap() {
                        let _ = block_on(session.update_media_properties())
                            .inspect_err(|e| log::warn!("Failed to update media properties: {e}"));
                    }
                    Ok(())
                }))
                .unwrap();

            let session_clone = Arc::clone(session_mutex);
            let timeline_properties_changed_token = wrt_session
                .TimelinePropertiesChanged(&TypedEventHandler::new(move |_, _| {
                    if let Some(session) = &mut *session_clone.lock().unwrap() {
                        let _ = block_on(session.update_timeline_properties()).inspect_err(|e| {
                            log::warn!("Failed to update timeline properties: {e}")
                        });
                    }
                    Ok(())
                }))
                .unwrap();

            session.set_event_tokens(EventTokens {
                media_properties_changed_token,
                playback_info_changed_token,
                timeline_properties_changed_token,
            });
        }
    }

    fn update_session(session_mutex: &Arc<Mutex<Option<MediaSessionStruct>>>) {
        let mut session = session_mutex.lock().unwrap();

        if let Some(session) = &mut *session {
            block_on(session.full_update());
        }
    }

    #[allow(clippy::await_holding_lock)]
    async fn update_session_async(session_mutex: &Arc<Mutex<Option<MediaSessionStruct>>>) {
        let mut session = session_mutex.lock().unwrap();

        if let Some(session) = &mut *session {
            session.full_update().await;
        }
    }

    fn create_session(manager: &Option<WRT_MediaManager>) -> Option<MediaSessionStruct> {
        if let Some(manager) = manager {
            let wrt_session = manager.GetCurrentSession();

            if let Ok(wrt_session) = wrt_session {
                log::info!("Found an existing session");

                let session = MediaSessionStruct::new(wrt_session);

                return Some(session);
            }
        }

        log::info!("No active sessions found");
        None
    }

    pub fn get_info(&self) -> MediaInfo {
        let session = self.session.lock().unwrap();

        if let Some(session) = &*session {
            return session.get_info();
        }

        MediaInfo::default()
    }
}

#[allow(clippy::await_holding_lock)]
impl MediaSessionControls for MediaSession {
    async fn pause(&self) -> crate::Result<()> {
        if let Some(session) = &*self.session.lock().unwrap() {
            session.pause().await?;
        }

        Ok(())
    }

    async fn play(&self) -> crate::Result<()> {
        if let Some(session) = &*self.session.lock().unwrap() {
            session.play().await?;
        }
        Ok(())
    }

    async fn toggle_pause(&self) -> crate::Result<()> {
        if let Some(session) = &*self.session.lock().unwrap() {
            session.toggle_pause().await?;
        }
        Ok(())
    }

    async fn stop(&self) -> crate::Result<()> {
        if let Some(session) = &*self.session.lock().unwrap() {
            session.stop().await?;
        }
        Ok(())
    }

    async fn next(&self) -> crate::Result<()> {
        if let Some(session) = &*self.session.lock().unwrap() {
            session.next().await?;
        }
        Ok(())
    }

    async fn prev(&self) -> crate::Result<()> {
        if let Some(session) = &*self.session.lock().unwrap() {
            session.prev().await?;
        }
        Ok(())
    }
}
