# MediaSession.rs

Educational project

Rust library to control / get metadata of music playback

Utilizes: `WinRT.Windows.Media.Control` API on Windows | `DBus/MPRIS` API on Linux

## Example

```rust
let player = media_session::MediaSession::new().await;

let info: media_session::MediaInfo = player.get_info();

println!("{:#?}", info);
```
```rust
MediaInfo {
    title: "St. Chroma (feat. Daniel Caesar)",
    artist: "Tyler, The Creator",
    album_title: "CHROMAKOPIA",
    album_artist: "Tyler, The Creator",
    duration: 197019000, // microseconds
    position: 5700398,   // microseconds
    state: "playing",
    cover_b64: <...>, // encoded (without data type)
    cover_raw: <...>, // file data (bytes)
}
```

## TODO

- [ ] Callback on update
- [ ] Parse type of image
- [ ] Make update on signal in unix imp
