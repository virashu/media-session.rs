use futures::executor::block_on;

mod media_info;
mod player;

use media_info::MediaInfo;
use player::Player;

fn update(info: MediaInfo) {
    print!(
        "\x1b[?25l\x1b[2J\x1b[1;1H\
        \n\t-> Title: \x1b[32m{}\x1b[0m\
        \n\t|  Artist: \x1b[32m{}\x1b[0m\
        \n\t|  Position: \x1b[32m{}\x1b[0m/\x1b[31m{}\x1b[0m\
        \n\t|  ~: \x1b[32m{}\x1b[0m\
        \n\t|  @: \x1b[32m{}\x1b[0m
        \x1b[?25h",
        info.title, info.artist, info.position, info.duration, info.pos_last_update, player::micros_since_epoch()
    );
}

async fn start() {
    let mut player = Player::new(update).await;

    player.create_session().await;

    // wait forever
    loop {
        player.update().await;

        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

fn main() {
    block_on(start());
}
