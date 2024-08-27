pub use crate::traits;
use crate::MediaInfo;
use dbus::blocking::{self, Connection, Proxy};
use dbus::strings::BusName;
use dbus::Path;
use dbus::{
    arg::{PropMap, RefArg},
    blocking::stdintf::org_freedesktop_dbus::Properties as _,
};
use std::time::Duration;

const DBUS_DEST: &str = "org.freedesktop.DBus";
const DBUS_PATH: &str = "/org/freedesktop/DBus";

const PLAYER_DEST: &'static str = "org.mpris.MediaPlayer2";
const PLAYER_PATH: &'static str = "/org/mpris/MediaPlayer2";

const TIMEOUT: Duration = Duration::new(5, 0);

pub struct MediaSession<'a> {
    bus: blocking::Connection,
    player: Option<blocking::Proxy<'a, &'a blocking::Connection>>,
    callback: Option<Box<dyn Fn(MediaInfo)>>,
}

impl<'a> MediaSession<'a> {
    pub async fn new() -> Self {
        let conn = blocking::Connection::new_session().unwrap();
        let mut session = Self {
            bus: conn,
            player: None,
            callback: None,
        };

        let player = session.create_session().await;
        session.player = player;

        session
    }

    fn get_dbus_proxy(&self) -> blocking::Proxy<'a, &blocking::Connection> {
        self.get_proxy(DBUS_DEST, "/")
    }

    fn get_proxy<D, P>(&self, dest: D, path: P) -> blocking::Proxy<'a, &blocking::Connection>
    where
        D: Into<BusName<'a>>,
        P: Into<Path<'a>>,
    {
        Proxy::<'a, &blocking::Connection> {
            connection: &self.bus,
            destination: dest.into(),
            path: path.into(),
            timeout: TIMEOUT,
        }
    }

    async fn create_session<'b>(&'b self) -> Option<Proxy<'b, &'b Connection>> {
        let proxy = self.get_dbus_proxy();

        let (names,): (Vec<String>,) = proxy.method_call(DBUS_DEST, "ListNames", ()).unwrap();

        let players: Vec<String> = names
            .iter()
            .filter(|s| s.starts_with(PLAYER_DEST))
            .map(|s_ref| s_ref.clone())
            .collect();

        let count = players.len();

        if count > 0 {
            println!("Players found: {}", count);
        } else {
            println!("No players found");
            return None;
        }

        for player in &players {
            println!("- {}", player);
        }

        // TODO: find a way to select the last updated player
        let selected_dest: String = players[0].clone();

        let player: Proxy<'b, &Connection> = self.get_proxy(selected_dest, PLAYER_PATH);

        Some(player)
    }
    fn get_data_internal(&self) {
        if let Some(player) = &self.player {
            let metadata: PropMap = player
                .get("org.mpris.MediaPlayer2.Player", "Metadata")
                .unwrap();

            for (key, value) in metadata.iter() {
                print!("  {}: ", key);
                print_refarg(&value);
            }
        }
    }
    pub async fn update(&mut self) {}

    pub fn get_info(&self) -> MediaInfo {
        todo!()
    }
    pub fn set_callback<F>(&mut self, callback: F)
    where
        F: Fn(MediaInfo) + 'static,
    {
        todo!()
    }
}

impl traits::MediaSessionControls for MediaSession<'_> {
    async fn next(&self) -> crate::Result<()> {
        todo!()
    }
    async fn pause(&self) -> crate::Result<()> {
        todo!()
    }
    async fn play(&self) -> crate::Result<()> {
        todo!()
    }
    async fn prev(&self) -> crate::Result<()> {
        todo!()
    }
    async fn stop(&self) -> crate::Result<()> {
        todo!()
    }
    async fn toggle_pause(&self) -> crate::Result<()> {
        todo!()
    }
}

fn print_refarg(value: &dyn RefArg) {
    if let Some(s) = value.as_str() {
        println!("{}", s);
    } else if let Some(i) = value.as_i64() {
        println!("{}", i);
    } else {
        println!("{:?}", value);
    }
}
