use futures::executor::block_on;

use std::cmp::max;
use std::time::Duration;

use media_session::media_info::MediaInfo;
use media_session::player::Player;

fn human_time(microsecs: i64) -> String {
    let secs = microsecs / 1_000_000;

    (secs / 60).to_string() + ":" + &format!("{:02}", secs % 60)
}

fn progress_bar_fira(pos_percent: usize) -> String {
    let center = "".repeat(max(pos_percent as i64 - 2, 0) as usize)
        + &"".repeat(max(100 - pos_percent as i64 - 2, 0) as usize);

    let start = if pos_percent >= 1 { "" } else { "" };
    let end = if pos_percent >= 100 { "" } else { "" };

    format!("{}{}{}", start, center, end)
}

fn progress_bar_ascii(pos_percent: usize) -> String {
    let center = "=".repeat(pos_percent) + &" ".repeat(100 - pos_percent);

    let start = "[";
    let end = "]";

    format!("{}{}{}", start, center, end)
}

fn update(info: MediaInfo) {
    let pos_percent: usize = (info.position as f64 / info.duration as f64 * 100.0) as usize;

    let progress_bar = progress_bar_fira(pos_percent); /* for Fira Code */
    // let progress_bar = progress_bar_ascii(pos_percent); /* for other fonts */
    let pos_str = human_time(info.position);
    let dur_str = human_time(info.duration);

    print!("\x1b[?25l\x1b[2J\x1b[1;1H");

    print!(
        "\
        \t\x1b[1;32m{}\x1b[22;0m\
        \n\t\x1b[3;2mby \x1b[32;22m{}\x1b[0m\x1b[23m\
        \n\n {} {} {}
        ",
        info.title, info.artist, pos_str, progress_bar, dur_str,
    );

    print!("\x1b[?25h");
}

async fn start() {
    let mut player = Player::new().await;
    
    player.set_callback(update).await;
    player.create_session().await;

    loop {
        player.update().await;

        std::thread::sleep(Duration::from_millis(100));
    }
}

fn main() {
    block_on(start());
}
