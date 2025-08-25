use std::{fs, time::Duration};

use base64::{display::Base64Display, engine::general_purpose::STANDARD as BASE64_STANDARD};
use dbus::{
    arg::{PropMap, RefArg},
    blocking,
    blocking::stdintf::org_freedesktop_dbus::Properties as _,
    strings::BusName,
    Path,
};

use crate::{traits, MediaInfo};

const DBUS_DEST: &str = "org.freedesktop.DBus";
// const DBUS_PATH: &str = "/org/freedesktop/DBus";

const PLAYER_PATH: &str = "/org/mpris/MediaPlayer2";

const PLAYER_INTERFACE: &str = "org.mpris.MediaPlayer2";
const PLAYER_INTERFACE_PLAYER: &str = "org.mpris.MediaPlayer2.Player";

const TIMEOUT: Duration = Duration::new(5, 0);

fn get_player_names(proxy: &Proxy) -> Vec<String> {
    let res: (Vec<String>,) = proxy.method_call(DBUS_DEST, "ListNames", ()).unwrap();
    res.0
}

fn select_player(proxy: &Proxy) -> Option<String> {
    let names = get_player_names(proxy);

    let players: Vec<String> = names
        .iter()
        .filter(|s| s.starts_with(PLAYER_INTERFACE))
        .cloned()
        .collect();

    let count = players.len();

    if count > 0 {
        tracing::info!("Found {} players", count);
        if count > 1 {
            players
                .iter()
                .enumerate()
                .for_each(|(i, p)| tracing::info!("  {i}) {p}"));
        }
        tracing::info!("Selected: {}", players[0]);
        return Some(players[0].clone());
    }

    None
}

fn get_proxy<'a, D, P>(dest: D, path: P) -> blocking::Proxy<'a, Box<blocking::Connection>>
where
    D: Into<BusName<'a>>,
    P: Into<Path<'a>>,
{
    let connection = Box::new(blocking::Connection::new_session().unwrap());

    blocking::Proxy::<'a, Box<blocking::Connection>> {
        destination: dest.into(),
        path: path.into(),
        timeout: TIMEOUT,
        connection,
    }
}

fn get_dbus_proxy<'a>() -> blocking::Proxy<'a, Box<blocking::Connection>> {
    get_proxy(DBUS_DEST, "/")
}

#[derive(Default)]
pub struct MediaSession {
    player: Option<blocking::Proxy<'static, Box<blocking::Connection>>>,
    media_info: Option<MediaInfo>,
    prev_cover_url: Option<String>,
    prev_cover_raw: Option<Vec<u8>>,
    prev_cover_b64: Option<String>,
}

type Proxy<'l> = blocking::Proxy<'l, Box<blocking::Connection>>;

impl MediaSession {
    #[must_use]
    pub fn new() -> Self {
        let player = Self::try_get_player_dest().map_or_else(
            || {
                tracing::info!("No players found");
                None
            },
            |player_dest| {
                let player = get_proxy(player_dest, PLAYER_PATH);
                Some(player)
            },
        );

        Self {
            player,
            ..Default::default()
        }
    }

    fn try_get_player_dest() -> Option<String> {
        let dbus_proxy = get_dbus_proxy();

        select_player(&dbus_proxy)
    }

    // fn get_data_internal(&self) {
    //     if let Some(player) = &self.player {
    //         let metadata: PropMap = player
    //             .get("org.mpris.MediaPlayer2.Player", "Metadata")
    //             .unwrap();

    //         for (k, value) in &metadata {
    //             if let Some(s) = value.as_str() {
    //                 tracing::info!("  {}:\t {}", k, s);
    //             } else if let Some(i) = value.as_i64() {
    //                 tracing::info!("  {}:\t {}", k, i);
    //             } else {
    //                 tracing::info!("  {}:\t {:?}", k, value);
    //             }
    //         }
    //     }
    // }

    fn update_player(&mut self) {
        // Check for player change
        let new_dest = Self::try_get_player_dest();
        let cur_player = &mut self.player;
        let cur_dest = cur_player.as_ref().map(|p| p.destination.to_string());

        if new_dest != cur_dest {
            if let Some(dest) = new_dest {
                *cur_player = Some(get_proxy(dest, PLAYER_PATH));
            }
        }
    }

    fn update_info(&mut self) {
        if let Some(player) = &self.player {
            // Error on player application close
            let metadata: Result<PropMap, dbus::Error> =
                player.get(PLAYER_INTERFACE_PLAYER, "Metadata");

            if metadata.is_err() {
                self.media_info = None;
                return;
            }

            let metadata: PropMap = metadata.unwrap();

            let position: Result<i64, dbus::Error> =
                player.get(PLAYER_INTERFACE_PLAYER, "Position");

            let state: Result<String, dbus::Error> =
                player.get(PLAYER_INTERFACE_PLAYER, "PlaybackStatus");

            let cover_raw: Option<Vec<u8>>;
            let cover_b64: Option<String>;

            if let Some(cover_url) = get_string(&metadata, "mpris:artUrl") {
                if cover_url.is_empty() {
                    cover_raw = None;
                    cover_b64 = None;
                } else {
                    tracing::info!("Cover url: {cover_url}");
                    let cover_url = cover_url.strip_prefix("file://").unwrap().to_string();
                    // cover_raw = self.get_cover_raw(cover_url.clone());
                    cover_raw = None;
                    cover_b64 = self.get_cover_b64(cover_url);
                }
            } else {
                cover_raw = None;
                cover_b64 = None;
            }

            self.media_info = Some(MediaInfo {
                title: get_string(&metadata, "xesam:title").unwrap_or_default(),
                artist: get_first_string(&metadata, "xesam:artist").unwrap_or_default(),
                duration: get_i64(&metadata, "mpris:length").unwrap_or_default(),
                position: position.unwrap_or_default(),
                state: state.map(|s| s.to_lowercase()).unwrap_or_default(),
                cover_raw: cover_raw.unwrap_or_default(),
                cover_b64: cover_b64.unwrap_or_else(|| String::from("Missing")),
                album_title: get_string(&metadata, "xesam:albumArtist").unwrap_or_default(),
                album_artist: get_string(&metadata, "xesam:album").unwrap_or_default(),
            });
        }
    }

    pub fn update(&mut self) {
        self.update_player();
        self.update_info();
    }

    #[must_use]
    pub fn get_info(&self) -> MediaInfo {
        self.media_info.clone().unwrap_or_default()
    }

    fn get_cover_raw(&mut self, cover_url: String) -> Option<Vec<u8>> {
        if let Some(prev_url) = &self.prev_cover_url {
            if *prev_url == cover_url {
                return self.prev_cover_raw.clone();
            }
        }

        {
            self.prev_cover_url = Some(cover_url.clone());
        }
        tracing::info!("Reading cover at: {}", cover_url);

        let cover_raw = fs::read(cover_url);

        if let Ok(c) = cover_raw {
            tracing::info!("Read cover; size: {} Bytes", c.len());
            return Some(c);
        }

        if let Err(e) = cover_raw {
            tracing::error!("Failed to read cover: {e}");
        }

        None
    }

    fn get_cover_b64(&mut self, cover_url: String) -> Option<String> {
        if let Some(prev_url) = &self.prev_cover_url {
            if *prev_url == cover_url {
                return self.prev_cover_b64.clone();
            }
        }

        {
            self.prev_cover_url = Some(cover_url.clone());
        }
        let cover_raw = fs::read(cover_url);

        if let Ok(c) = cover_raw {
            tracing::info!("B64 cover read success");
            let b64 = Base64Display::new(&c, &BASE64_STANDARD).to_string();
            // let b64 = BASE64_STANDARD.encode(c);
            self.prev_cover_b64 = Some(b64.clone());

            return Some(b64);
        }

        tracing::warn!("Failed to read file for b64!");

        None
    }
}

fn action(player_opt: Option<&Proxy>, command: &str) -> crate::Result<()> {
    if let Some(player) = player_opt {
        return player
            .method_call(PLAYER_INTERFACE_PLAYER, command, ())
            .map_err(crate::error::Error::from);
    }

    Ok(())
}

impl traits::MediaSessionControls for MediaSession {
    fn next(&self) -> crate::Result<()> {
        action(self.player.as_ref(), "Next")
    }
    fn pause(&self) -> crate::Result<()> {
        action(self.player.as_ref(), "Pause")
    }
    fn play(&self) -> crate::Result<()> {
        action(self.player.as_ref(), "Play")
    }
    fn prev(&self) -> crate::Result<()> {
        action(self.player.as_ref(), "Previous")
    }
    fn stop(&self) -> crate::Result<()> {
        action(self.player.as_ref(), "Stop")
    }
    fn toggle_pause(&self) -> crate::Result<()> {
        action(self.player.as_ref(), "PlayPause")
    }
}

fn get_i64<StringLike: Into<String>>(meta: &PropMap, key: StringLike) -> Option<i64> {
    refarg_to_i64(meta.get(&key.into())?)
}

fn get_string<StringLike: Into<String>>(meta: &PropMap, key: StringLike) -> Option<String> {
    refarg_to_string(meta.get(&key.into())?)
}

fn get_first_string<StringLike: Into<String>>(meta: &PropMap, key: StringLike) -> Option<String> {
    let a = meta.get(&key.into())?;
    let b = refarg_first(a);
    refarg_to_string(b)
}

fn refarg_to_string(value: &dyn RefArg) -> Option<String> {
    Some(value.as_str()?.to_string())
}

fn refarg_to_i64(value: &dyn RefArg) -> Option<i64> {
    value.as_i64()
}

fn refarg_first(value: &dyn RefArg) -> &dyn RefArg {
    value
        .as_iter()
        .unwrap()
        .next()
        .unwrap()
        .as_iter()
        .unwrap()
        .next()
        .unwrap()
}
