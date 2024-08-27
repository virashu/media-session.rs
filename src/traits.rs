// use crate::MediaInfo;

// pub trait MediaSession {
//     async fn update(&mut self);

//     fn set_callback<F>(&mut self, callback: F)
//     where
//         F: Fn(MediaInfo) + 'static;

//     fn get_info(&self) -> MediaInfo;
// }

pub trait MediaSessionControls {
    async fn toggle_pause(&self) -> crate::Result<()>;
    async fn pause(&self) -> crate::Result<()>;
    async fn play(&self) -> crate::Result<()>;
    async fn stop(&self) -> crate::Result<()>;
    async fn next(&self) -> crate::Result<()>;
    async fn prev(&self) -> crate::Result<()>;
}
