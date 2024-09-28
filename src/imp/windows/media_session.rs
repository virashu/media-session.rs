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
                let mut s = session.lock().unwrap();
                *s = Self::create_session(manager);
                Ok(())
            }))
            .unwrap();

        self.event_token = Some(token);
    }

    fn setup_session_listeners(&self) {
        let mut session_opt = self.session.lock().unwrap();

        if let Some(session) = &mut *session_opt {
            let wrt_session = session.get_session();

            let session_clone = Arc::clone(&self.session);
            let playback_info_changed_token = wrt_session
                .PlaybackInfoChanged(&TypedEventHandler::new(move |_, _| {
                    if let Some(session) = &mut *session_clone.lock().unwrap() {
                        block_on(session.update_playback_info());
                    }
                    Ok(())
                }))
                .unwrap();

            let session_clone = Arc::clone(&self.session);
            let media_properties_changed_token = wrt_session
                .MediaPropertiesChanged(&TypedEventHandler::new(move |_, _| {
                    if let Some(session) = &mut *session_clone.lock().unwrap() {
                        block_on(session.update_media_properties()).;
                    }
                    Ok(())
                }))
                .unwrap();

            let session_clone = Arc::clone(&self.session);
            let timeline_properties_changed_token = wrt_session
                .TimelinePropertiesChanged(&TypedEventHandler::new(move |_, _| {
                    if let Some(session) = &mut *session_clone.lock().unwrap() {
                        block_on(session.update_timeline_properties());
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

    fn create_session(manager: &Option<WRT_MediaManager>) -> Option<MediaSessionStruct> {
        if let Some(manager) = manager {
            let wrt_session = manager.GetCurrentSession();

            if let Ok(wrt_session) = wrt_session {
                let mut session = MediaSessionStruct::new(wrt_session);

                // init it

                session.init();
            }
        }

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
