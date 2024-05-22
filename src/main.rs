use futures::executor::block_on;

mod media_info;
mod player;

use player::Player;

use crate::media_info::MediaInfo;

fn update(info: MediaInfo) {
    println!("Title: \x1b[32m{}\x1b[0m", info.title);
}

async fn start() {
    let mut player = Player::new(update).await;

    player.create_session().await;

    // wait forever
    loop {}
}

fn main() {
    block_on(start());
}
