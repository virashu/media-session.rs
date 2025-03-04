pub use crate::traits;
use crate::MediaInfo;
use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use dbus::blocking;
use dbus::strings::BusName;
use dbus::Path;
use dbus::{
    arg::{PropMap, RefArg},
    blocking::stdintf::org_freedesktop_dbus::Properties as _,
};
use std::fs;
use std::time::Duration;

const DBUS_DEST: &str = "org.freedesktop.DBus";
// const DBUS_PATH: &str = "/org/freedesktop/DBus";

const PLAYER_PATH: &str = "/org/mpris/MediaPlayer2";

const PLAYER_INTERFACE: &str = "org.mpris.MediaPlayer2";
const PLAYER_INTERFACE_PLAYER: &str = "org.mpris.MediaPlayer2.Player";

const TIMEOUT: Duration = Duration::new(5, 0);

fn get_player_names(proxy: Proxy) -> Vec<String> {
    let res: (Vec<String>,) = proxy.method_call(DBUS_DEST, "ListNames", ()).unwrap();
    res.0
}

fn select_player(proxy: Proxy) -> Option<String> {
    let names = get_player_names(proxy);

    let players: Vec<String> = names
        .iter()
        .filter(|s| s.starts_with(PLAYER_INTERFACE))
        .cloned()
        .collect();

    let count = players.len();

    if count > 0 {
        log::info!("Found {} players", count);
        if count > 1 {
            players
                .iter()
                .enumerate()
                .for_each(|(i, p)| log::info!("  {i}) {p}"));
        }
        log::info!("Selected: {}", players[0]);
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

pub struct MediaSession<'a> {
    player: Option<blocking::Proxy<'a, Box<blocking::Connection>>>,
    prev_cover_url: Option<String>,
    prev_cover_raw: Option<Vec<u8>>,
    prev_cover_b64: Option<String>,
}

type Proxy<'l> = blocking::Proxy<'l, Box<blocking::Connection>>;

impl<'a> MediaSession<'a> {
    pub async fn new() -> Self {
        let mut session = Self {
            player: None,
            prev_cover_url: None,
            prev_cover_b64: None,
            prev_cover_raw: None,
        };

        let dbus_proxy = get_dbus_proxy();

        if let Some(player_dest) = select_player(dbus_proxy) {
            let player = get_proxy(player_dest, PLAYER_PATH);
            session.player = Some(player);
        } else {
            log::info!("No players found");
        }

        session.get_data_internal();
        log::info!("");
        log::info!("{:#?}", session.get_info());

        session
    }

    fn get_data_internal(&self) {
        if let Some(player) = &self.player {
            let metadata: PropMap = player
                .get("org.mpris.MediaPlayer2.Player", "Metadata")
                .unwrap();

            for (k, value) in metadata.iter() {
                print!("  {}: ", k);

                if let Some(s) = value.as_str() {
                    log::info!("  {}:\t {}", k, s);
                } else if let Some(i) = value.as_i64() {
                    log::info!("  {}:\t {}", k, i);
                } else {
                    log::info!("  {}:\t {:?}", k, value);
                }
            }
        }
    }

    pub fn get_info(&mut self) -> MediaInfo {
        if let Some(player) = &self.player {
            let metadata: PropMap = player.get(PLAYER_INTERFACE_PLAYER, "Metadata").unwrap();

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
                    let cover_url = cover_url.strip_prefix("file://").unwrap().to_string();
                    // cover_raw = self.get_cover_raw(cover_url.clone());
                    cover_raw = None;
                    cover_b64 = self.get_cover_b64(cover_url.clone());
                }
            } else {
                cover_raw = None;
                cover_b64 = None;
            }

            return MediaInfo {
                title: get_string(&metadata, "xesam:title").unwrap_or_default(),
                artist: get_first_string(&metadata, "xesam:artist").unwrap_or_default(),
                duration: get_i64(&metadata, "mpris:length").unwrap_or_default(),
                position: position.unwrap_or_default(),
                state: state.map(|s| s.to_lowercase()).unwrap_or_default(),
                cover_raw: cover_raw.unwrap_or_default(),
                cover_b64: cover_b64.unwrap_or(String::from("Missing")),
                album_title: get_string(&metadata, "xesam:albumArtist").unwrap_or_default(),
                album_artist: get_string(&metadata, "xesam:album").unwrap_or_default(),
            };
        }

        MediaInfo::default()
    }

    fn get_cover_raw(&mut self, cover_url: String) -> Option<Vec<u8>> {
        if let Some(prev_url) = &self.prev_cover_url {
            if *prev_url == cover_url {
                return self.prev_cover_raw.clone();
            }
        }

        self.prev_cover_url = Some(cover_url.clone());

        log::info!("Reading cover at: {}", cover_url);

        let cover_raw = fs::read(cover_url);

        if let Ok(c) = cover_raw {
            log::info!("Read cover; size: {} Bytes", c.len());
            return Some(c);
        }

        if let Err(e) = cover_raw {
            log::error!("Failed to read cover: {e}");
        }

        None
    }

    fn get_cover_b64(&mut self, cover_url: String) -> Option<String> {
        if let Some(prev_url) = &self.prev_cover_url {
            if *prev_url == cover_url {
                return self.prev_cover_b64.clone();
            }
        }

        self.prev_cover_url = Some(cover_url.clone());

        let cover_raw = fs::read(cover_url);

        if let Ok(c) = cover_raw {
            log::info!("B64 cover read success");
            let b64 =
                base64::display::Base64Display::new(&c, &base64::engine::general_purpose::STANDARD)
                    .to_string();
            // let b64 = BASE64_STANDARD.encode(c);
            self.prev_cover_b64 = Some(b64.clone());

            return Some(b64);
        }

        log::warn!("Failed to read file for b64!");

        None
    }
}

fn action(player_opt: &Option<Proxy>, command: &str) -> crate::Result<()> {
    if let Some(player) = &player_opt {
        return player
            .method_call(PLAYER_INTERFACE_PLAYER, command, ())
            .map_err(crate::error::Error::from);
    }

    Ok(())
}

impl traits::MediaSessionControls for MediaSession<'_> {
    async fn next(&self) -> crate::Result<()> {
        action(&self.player, "Next")
    }
    async fn pause(&self) -> crate::Result<()> {
        action(&self.player, "Pause")
    }
    async fn play(&self) -> crate::Result<()> {
        action(&self.player, "Play")
    }
    async fn prev(&self) -> crate::Result<()> {
        action(&self.player, "Previous")
    }
    async fn stop(&self) -> crate::Result<()> {
        action(&self.player, "Stop")
    }
    async fn toggle_pause(&self) -> crate::Result<()> {
        action(&self.player, "PlayPause")
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
