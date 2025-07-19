use std::{thread, time::Duration};

use media_session::MediaSession;

fn main() {
    #[cfg(feature = "tracing-subscriber")]
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let mut player = MediaSession::new();

    loop {
        player.update();
        thread::sleep(Duration::from_secs(1));
    }
}
