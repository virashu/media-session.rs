use std::{thread, time::Duration};

use futures::executor::block_on;

use media_session::MediaSession;

async fn start() {
    #[cfg(feature = "colog")]
    colog::default_builder()
        .filter(None, log::LevelFilter::Debug)
        .init();

    let _player = MediaSession::new().await;

    loop {
        thread::sleep(Duration::from_secs(1));
    }
}

fn main() {
    block_on(start());
}
