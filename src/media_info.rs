#[derive(Clone, Debug)]
pub struct MediaInfo {
    pub title: String,
    pub artist: String,

    pub album_title: String,
    pub album_artist: String,

    pub duration: i64,
}

impl MediaInfo {
    pub fn new() -> MediaInfo {
        MediaInfo {
            title: String::new(),
            artist: String::new(),
            
            album_title: String::new(),
            album_artist: String::new(),

            duration: 0,
        }
    }
}
