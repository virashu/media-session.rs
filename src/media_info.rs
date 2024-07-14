use crate::media_status::MediaStatus;

#[derive(Clone, Debug)]
pub struct MediaInfo {
    pub title: String,
    pub artist: String,

    pub album_title: String,
    pub album_artist: String,

    pub duration: i64,
    pub position: i64,

    pub pos_last_update: i64,
    pub pos_raw: i64,

    pub status: String, // stopped, paused, playing
    pub is_playing: bool,
}

impl MediaInfo {
    pub fn new() -> MediaInfo {
        MediaInfo {
            title: String::new(),
            artist: String::new(),
            
            album_title: String::new(),
            album_artist: String::new(),

            duration: 0,
            position: 0,

            pos_last_update: 0,
            pos_raw: 0,

            status: MediaStatus::Stopped.to_string(),
            is_playing: false,
        }
    }
}
