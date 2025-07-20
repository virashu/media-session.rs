use std::sync::mpsc::{channel, Receiver, Sender};

use windows::{
    Foundation::{EventRegistrationToken as WRT_EventToken, TypedEventHandler as WRT_EventHandler},
    Media::Control::GlobalSystemMediaTransportControlsSessionManager as WRT_MediaManager,
};

use crate::{traits::MediaSessionControls, MediaInfo};

use super::session::Session;

enum ManagerEvent {
    CurrentSessionChanged,
}

struct ManagerEventTokens {
    current_session_changed: WRT_EventToken,
}

pub struct MediaSession {
    runtime: tokio::runtime::Runtime,

    manager: WRT_MediaManager,
    manager_event_channel: (Sender<ManagerEvent>, Receiver<ManagerEvent>),
    manager_event_tokens: ManagerEventTokens,

    session: Option<Session>,
}

impl MediaSession {
    #[allow(clippy::new_without_default, clippy::missing_panics_doc)]
    #[must_use]
    pub fn new() -> Self {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        let manager = runtime
            .block_on(WRT_MediaManager::RequestAsync().unwrap())
            .unwrap();

        let manager_event_channel = channel();
        let manager_event_tokens =
            Self::setup_manager_events(&manager, manager_event_channel.0.clone());

        let mut self_ = Self {
            runtime,
            manager,
            manager_event_channel,
            manager_event_tokens,
            session: None,
        };

        self_.setup_session();
        self_
    }

    fn setup_session(&mut self) {
        let Ok(wrt_session) = self.manager.GetCurrentSession() else {
            return;
        };

        let mut session = Session::new(wrt_session);
        self.runtime.block_on(session.update_all());

        self.session = Some(session);
    }

    fn setup_manager_events(
        manager: &WRT_MediaManager,
        event_sender: Sender<ManagerEvent>,
    ) -> ManagerEventTokens {
        let token = manager
            .CurrentSessionChanged(&WRT_EventHandler::new(move |_, _| {
                event_sender
                    .send(ManagerEvent::CurrentSessionChanged)
                    .unwrap();
                Ok(())
            }))
            .unwrap();

        ManagerEventTokens {
            current_session_changed: token,
        }
    }

    fn process_manager_events(&mut self) {
        while let Ok(event) = self.manager_event_channel.1.try_recv() {
            match event {
                ManagerEvent::CurrentSessionChanged => self.setup_session(),
            }
        }
    }

    pub fn update(&mut self) {
        self.process_manager_events();

        if let Some(s) = self.session.as_mut() {
            self.runtime.block_on(s.update());
        }
    }

    pub fn get_info(&self) -> MediaInfo {
        self.session
            .as_ref()
            .map_or_else(MediaInfo::default, super::session::Session::get_info)
    }
}

impl MediaSessionControls for MediaSession {
    fn next(&self) -> crate::Result<()> {
        if let Some(session) = &self.session {
            self.runtime.block_on(session.next())?;
        }
        Ok(())
    }
    fn pause(&self) -> crate::Result<()> {
        if let Some(session) = &self.session {
            self.runtime.block_on(session.pause())?;
        }
        Ok(())
    }
    fn play(&self) -> crate::Result<()> {
        if let Some(session) = &self.session {
            self.runtime.block_on(session.play())?;
        }
        Ok(())
    }
    fn prev(&self) -> crate::Result<()> {
        if let Some(session) = &self.session {
            self.runtime.block_on(session.prev())?;
        }
        Ok(())
    }
    fn stop(&self) -> crate::Result<()> {
        if let Some(session) = &self.session {
            self.runtime.block_on(session.stop())?;
        }
        Ok(())
    }
    fn toggle_pause(&self) -> crate::Result<()> {
        if let Some(session) = &self.session {
            self.runtime.block_on(session.toggle_pause())?;
        }
        Ok(())
    }
}

impl Drop for MediaSession {
    fn drop(&mut self) {
        self.manager
            .RemoveCurrentSessionChanged(self.manager_event_tokens.current_session_changed)
            .unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run() {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .init();

        let mut player = MediaSession::new();
        player.update();

        println!("{:#?}", player.get_info());
    }
}
