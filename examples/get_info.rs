use futures::executor::block_on;

use media_session::MediaSession;

async fn start() {
    let player = MediaSession::new().await;

    let info = player.get_info().await;

    println!("{info:#?}");
}

fn main() {
    block_on(start());
}
