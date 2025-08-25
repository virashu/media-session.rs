pub trait MediaSessionControls {
    fn toggle_pause(&self) -> crate::Result<()>;
    fn pause(&self) -> crate::Result<()>;
    fn play(&self) -> crate::Result<()>;
    fn stop(&self) -> crate::Result<()>;
    fn next(&self) -> crate::Result<()>;
    fn prev(&self) -> crate::Result<()>;
}
