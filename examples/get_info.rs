use media_session::MediaSession;

fn main() {
    let player = MediaSession::new();
    let info = player.get_info();

    println!("{info:#?}");
}
