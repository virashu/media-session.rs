use std::sync::Arc;

use tokio::{runtime::Runtime, sync::Mutex};
use windows::{
    Foundation::{EventRegistrationToken, TypedEventHandler},
    Media::Control::GlobalSystemMediaTransportControlsSessionManager as WRT_MediaManager,
};

use super::media_session_struct::{EventTokens, MediaSessionStruct};
use crate::{traits::MediaSessionControls, MediaInfo};

pub struct MediaSession {
    rt: Arc<Runtime>,
    manager: WRT_MediaManager,
    session: Arc<Mutex<Option<MediaSessionStruct>>>,
    event_token: Option<EventRegistrationToken>,
}

#[allow(clippy::new_without_default)]
impl MediaSession {
    pub fn new() -> Self {
        let rt = Arc::new(
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap(),
        );

        let manager = rt
            .block_on(WRT_MediaManager::RequestAsync().unwrap())
            .unwrap();

        let session_opt = Self::create_session(Some(&manager));
        let session = Arc::new(Mutex::new(session_opt));

        Self::update_session(&rt, &session);

        let mut media_session = Self {
            rt,
            manager,
            session,
            event_token: None,
        };

        media_session.setup_manager_listeners();

        media_session
    }

    fn setup_manager_listeners(&mut self) {
        let session = Arc::clone(&self.session);
        let rt = Arc::clone(&self.rt);

        let token = self
            .manager
            .CurrentSessionChanged(&TypedEventHandler::new(
                move |manager: &Option<WRT_MediaManager>, _| {
                    rt.block_on(async {
                        *session.lock().await = Self::create_session(manager.as_ref());
                    });

                    Self::setup_session_listeners(&rt, &session);
                    Self::update_session(&rt, &session);

                    Ok(())
                },
            ))
            .unwrap();

        self.event_token = Some(token);
    }

    fn setup_session_listeners(
        rt: &Arc<Runtime>,
        session_mutex: &Arc<Mutex<Option<MediaSessionStruct>>>,
    ) {
        let mut session_opt = rt.block_on(session_mutex.lock());

        if let Some(session) = &mut *session_opt {
            let wrt_session = session.get_session();

            let session_clone = Arc::clone(session_mutex);
            let rt_clone = Arc::clone(rt);
            let playback_info_changed_token = wrt_session
                .PlaybackInfoChanged(&TypedEventHandler::new(move |_, _| {
                    rt_clone.block_on(async {
                        if let Some(session) = &mut *session_clone.lock().await {
                            _ = session
                                .update_playback_info()
                                .inspect_err(|e| tracing::warn!("Failed to update playback info: {e}"));
                        }
                    });
                    Ok(())
                }))
                .unwrap();

            let session_clone = Arc::clone(session_mutex);
            let rt_clone = Arc::clone(rt);
            let media_properties_changed_token = wrt_session
                .MediaPropertiesChanged(&TypedEventHandler::new(move |_, _| {
                    rt_clone.block_on(async {
                        if let Some(session) = &mut *session_clone.lock().await {
                            _ = session.update_media_properties().await.inspect_err(|e| {
                                tracing::warn!("Failed to update media properties: {e}");
                            });
                        }
                    });
                    Ok(())
                }))
                .unwrap();

            let session_clone = Arc::clone(session_mutex);
            let rt_clone = Arc::clone(rt);
            let timeline_properties_changed_token = wrt_session
                .TimelinePropertiesChanged(&TypedEventHandler::new(move |_, _| {
                    rt_clone.block_on(async {
                        if let Some(session) = &mut *session_clone.lock().await {
                            _ = session.update_timeline_properties().inspect_err(|e| {
                                tracing::warn!("Failed to update timeline properties: {e}");
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

    fn update_session(rt: &Runtime, session: &Arc<Mutex<Option<MediaSessionStruct>>>) {
        rt.block_on(async {
            let mut session = session.lock().await;

            if let Some(session) = &mut *session {
                session.full_update().await;
            }
        });
    }

    fn create_session(manager: Option<&WRT_MediaManager>) -> Option<MediaSessionStruct> {
        if let Some(manager) = manager {
            let wrt_session = manager.GetCurrentSession();

            if let Ok(wrt_session) = wrt_session {
                tracing::info!("Found an existing session");

                let session = MediaSessionStruct::new(wrt_session);

                return Some(session);
            }
        }

        tracing::info!("No active sessions found");
        None
    }

    #[must_use]
    pub fn get_info(&self) -> MediaInfo {
        let session = self.rt.block_on(self.session.lock());

        if let Some(session) = &*session {
            return session.get_info();
        }

        MediaInfo::default()
    }
}

impl MediaSessionControls for MediaSession {
    fn pause(&self) -> crate::Result<()> {
        let opt = self.rt.block_on(self.session.lock());
        if let Some(session) = &*opt {
            self.rt.block_on(session.pause())?;
        }
        Ok(())
    }

    fn play(&self) -> crate::Result<()> {
        let opt = self.rt.block_on(self.session.lock());
        if let Some(session) = &*opt {
            self.rt.block_on(session.play())?;
        }
        Ok(())
    }

    fn toggle_pause(&self) -> crate::Result<()> {
        let opt = self.rt.block_on(self.session.lock());
        if let Some(session) = &*opt {
            self.rt.block_on(session.toggle_pause())?;
        }
        Ok(())
    }

    fn stop(&self) -> crate::Result<()> {
        let opt = self.rt.block_on(self.session.lock());
        if let Some(session) = &*opt {
            self.rt.block_on(session.stop())?;
        }
        Ok(())
    }

    fn next(&self) -> crate::Result<()> {
        let opt = self.rt.block_on(self.session.lock());
        if let Some(session) = &*opt {
            self.rt.block_on(session.next())?;
        }
        Ok(())
    }

    fn prev(&self) -> crate::Result<()> {
        let opt = self.rt.block_on(self.session.lock());
        if let Some(session) = &*opt {
            self.rt.block_on(session.prev())?;
        }
        Ok(())
    }
}
