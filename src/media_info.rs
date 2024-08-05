use crate::playback_state::PlaybackState;

#[derive(Clone, Debug)]
pub struct MediaInfo {
    pub title: String,
    pub artist: String,

    pub album_title: String,
    pub album_artist: String,

    pub duration: i64,
    pub position: i64,

    pub state: String, // stopped, paused, playing
    pub is_playing: bool,
    
    pub playback_rate: f64,
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

            state: PlaybackState::Stopped.into(),
            is_playing: false,

            playback_rate: 1.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct MediaInfoInternal {
    pub info: MediaInfo,
    pub pos_last_update: i64,
    pub pos_raw: i64,
}

impl MediaInfoInternal {
    pub fn new() -> MediaInfoInternal {
        MediaInfoInternal {
            info: MediaInfo::new(),
            pos_last_update: 0,
            pos_raw: 0,
        }
    }
}
