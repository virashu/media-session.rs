use futures::{executor::block_on, lock::Mutex};
use windows::{
    Foundation::{EventRegistrationToken, TypedEventHandler},
    Media::Control::GlobalSystemMediaTransportControlsSessionManager as WRT_MediaManager,
};

use std::sync::Arc;

use super::media_session_struct::{EventTokens, MediaSessionStruct};
use crate::{traits::MediaSessionControls, MediaInfo};

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
                block_on(async {
                    let mut s = session.lock().await;
                    *s = Self::create_session(manager);

                    Self::setup_session_listeners(&session).await;
                    Self::update_session(&session).await;
                });

                Ok(())
            }))
            .unwrap();

        self.event_token = Some(token);
    }

    async fn setup_session_listeners(session_mutex: &Arc<Mutex<Option<MediaSessionStruct>>>) {
        let mut session_opt = session_mutex.lock().await;

        if let Some(session) = &mut *session_opt {
            let wrt_session = session.get_session();

            let session_clone = Arc::clone(session_mutex);
            let playback_info_changed_token = wrt_session
                .PlaybackInfoChanged(&TypedEventHandler::new(move |_, _| {
                    block_on(async {
                        if let Some(session) = &mut *session_clone.lock().await {
                            _ = session
                                .update_playback_info()
                                .inspect_err(|e| log::warn!("Failed to update playback info: {e}"));
                        }
                    });
                    Ok(())
                }))
                .unwrap();

            let session_clone = Arc::clone(session_mutex);
            let media_properties_changed_token = wrt_session
                .MediaPropertiesChanged(&TypedEventHandler::new(move |_, _| {
                    block_on(async {
                        if let Some(session) = &mut *session_clone.lock().await {
                            _ = session.update_media_properties().await.inspect_err(|e| {
                                log::warn!("Failed to update media properties: {e}");
                            });
                        }
                    });
                    Ok(())
                }))
                .unwrap();

            let session_clone = Arc::clone(session_mutex);
            let timeline_properties_changed_token = wrt_session
                .TimelinePropertiesChanged(&TypedEventHandler::new(move |_, _| {
                    block_on(async {
                        if let Some(session) = &mut *session_clone.lock().await {
                            _ = session.update_timeline_properties().inspect_err(|e| {
                                log::warn!("Failed to update timeline properties: {e}");
                            });
                        }
                    });
                    Ok(())
                }))
                .unwrap();

            session.set_event_tokens(EventTokens {
                playback_info: playback_info_changed_token,
                media_properties: media_properties_changed_token,
                timeline_properties: timeline_properties_changed_token,
            });
        }
    }

    async fn update_session(session_mutex: &Arc<Mutex<Option<MediaSessionStruct>>>) {
        let mut session = session_mutex.lock().await;

        if let Some(session) = &mut *session {
            block_on(session.full_update());
        }
    }

    async fn update_session_async(session_mutex: &Arc<Mutex<Option<MediaSessionStruct>>>) {
        let mut session = session_mutex.lock().await;

        if let Some(session) = &mut *session {
            session.full_update().await;
        }
    }

    #[allow(clippy::ref_option, reason = "used like this by WinRT")]
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

    pub async fn get_info(&self) -> MediaInfo {
        let session = self.session.lock().await;

        if let Some(session) = &*session {
            return session.get_info();
        }

        MediaInfo::default()
    }
}

impl MediaSessionControls for MediaSession {
    async fn pause(&self) -> crate::Result<()> {
        if let Some(session) = &*self.session.lock().await {
            session.pause().await?;
        }

        Ok(())
    }

    async fn play(&self) -> crate::Result<()> {
        if let Some(session) = &*self.session.lock().await {
            session.play().await?;
        }
        Ok(())
    }

    async fn toggle_pause(&self) -> crate::Result<()> {
        if let Some(session) = &*self.session.lock().await {
            session.toggle_pause().await?;
        }
        Ok(())
    }

    async fn stop(&self) -> crate::Result<()> {
        if let Some(session) = &*self.session.lock().await {
            session.stop().await?;
        }
        Ok(())
    }

    async fn next(&self) -> crate::Result<()> {
        if let Some(session) = &*self.session.lock().await {
            session.next().await?;
        }
        Ok(())
    }

    async fn prev(&self) -> crate::Result<()> {
        if let Some(session) = &*self.session.lock().await {
            session.prev().await?;
        }
        Ok(())
    }
}
