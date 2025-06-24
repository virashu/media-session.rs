// use crate::MediaInfo;

// pub trait MediaSession {
//     async fn update(&mut self);

//     fn set_callback<F>(&mut self, callback: F)
//     where
//         F: Fn(MediaInfo) + 'static;

//     fn get_info(&self) -> MediaInfo;
// }

pub trait MediaSessionControls {
    fn toggle_pause(&self) -> crate::Result<()>;
    fn pause(&self) -> crate::Result<()>;
    fn play(&self) -> crate::Result<()>;
    fn stop(&self) -> crate::Result<()>;
    fn next(&self) -> crate::Result<()>;
    fn prev(&self) -> crate::Result<()>;
}
