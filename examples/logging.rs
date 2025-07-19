use std::{thread, time::Duration};

use media_session::MediaSession;

fn main() {
    #[cfg(feature = "colog")]
    colog::default_builder()
        .filter(None, log::LevelFilter::Debug)
        .init();

    let _player = MediaSession::new();

    loop {
        thread::sleep(Duration::from_secs(1));
    }
}
