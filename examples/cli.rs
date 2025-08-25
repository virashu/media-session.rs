#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]

use std::{
    io::{stdout, Write},
    time::Duration,
};

use media_session::{MediaInfo, MediaSession};

fn human_time(microsecs: i64) -> String {
    let secs = microsecs / 1_000_000;

    format!("{}:{:02}", secs / 60, secs % 60)
}

#[cfg(feature = "powerfont")]
fn progress_bar(pos_percent: usize) -> String {
    let center = "".repeat(std::cmp::max(pos_percent as i64 - 2, 0) as usize)
        + &"".repeat(std::cmp::max(100 - pos_percent as i64 - 2, 0) as usize);

    let start = if pos_percent >= 1 { "" } else { "" };
    let end = if pos_percent >= 100 { "" } else { "" };

    format!("{start}{center}{end}")
}

#[cfg(not(feature = "powerfont"))]
fn progress_bar(pos_percent: usize) -> String {
    let center = "=".repeat(pos_percent) + &" ".repeat(100 - pos_percent);

    let start = "[";
    let end = "]";

    format!("{start}{center}{end}")
}

fn update(info: MediaInfo) {
    let pos_percent: usize = (info.position as f64 / info.duration as f64 * 100.0) as usize;

    let progress_bar = progress_bar(pos_percent);
    let pos_str = human_time(info.position);
    let dur_str = human_time(info.duration);

    let title = info.title;
    let artist = info.artist;

    let mut lock = stdout().lock();

    write!(lock, "\x1b[2J\x1b[H").unwrap(); /* fast clear */
    write!(
        lock,
        "       \x1b[1;32m{title}\x1b[22;0m\
        \n       \x1b[2;3;49mby \x1b[32;22m{artist}\x1b[0m\x1b[23m\
        \n\n {pos_str:>5} {progress_bar} {dur_str:>5}
        "
    )
    .unwrap();

    lock.flush().unwrap();
}

fn main() {
    // print!("\x1b[?25l");

    let mut player = MediaSession::new();

    loop {
        player.update();
        update(player.get_info());

        std::thread::sleep(Duration::from_millis(100));
    }

    // print!("\x1b[?25h");
}
