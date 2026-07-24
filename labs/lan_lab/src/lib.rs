//! Resettable real-UDP proof for the authoritative LAN server/client seam.

use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use observed_authoring::RuntimeHexCatalog;
use observed_content::ArchitectureRegister;
use observed_net::lan::LanClient;
use observed_server::{ServerConfig, ServerHandle};

#[derive(Resource)]
pub struct LanLabRuntime {
    server: ServerHandle,
    client: LanClient,
    ready_sent: bool,
    reset_count: u32,
    status: String,
}

impl LanLabRuntime {
    fn start(reset_count: u32) -> Result<Self, String> {
        let tile_dir = tile_dir();
        let slugs = ArchitectureRegister::ALL.map(ArchitectureRegister::slug);
        let catalog = RuntimeHexCatalog::load(&tile_dir, &slugs)?;
        let mut config = ServerConfig::default();
        config.bind = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
        config.discovery = false;
        config.name = "LAN Lab".to_string();
        config.tile_dir = tile_dir;
        let server = ServerHandle::spawn(config)?;
        let client = LanClient::connect(
            server.address,
            1,
            None,
            None,
            catalog.simulation_content_hash,
        )
        .map_err(|error| error.to_string())?;
        Ok(Self {
            server,
            client,
            ready_sent: false,
            reset_count,
            status: "connecting over real UDP loopback".to_string(),
        })
    }
}

#[derive(Component)]
struct StatusText;

pub struct LanLabPlugin;

impl Plugin for LanLabPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(LanLabRuntime::start(0).expect("LAN lab starts"))
            .add_systems(Startup, setup)
            .add_systems(Update, (drive, reset, draw).chain());
    }
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.spawn((
        StatusText,
        Text::new("LAN lab starting..."),
        TextFont {
            font_size: 22.0,
            ..default()
        },
        TextColor(Color::srgb(0.45, 0.92, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(36.0),
            top: Val::Px(32.0),
            ..default()
        },
    ));
}

fn drive(mut runtime: ResMut<LanLabRuntime>) {
    runtime.client.poll();
    if runtime.client.token.is_some() && !runtime.ready_sent {
        runtime.ready_sent = runtime.client.set_ready(true).is_ok();
    }
    let player = runtime
        .client
        .player
        .map_or("--".to_string(), |player| player.label());
    let team = runtime
        .client
        .team
        .map_or("--".to_string(), |team| team.label());
    let phase = runtime
        .client
        .phase
        .map_or("CONNECTING".to_string(), |phase| format!("{phase:?}"));
    let seats = runtime
        .client
        .lobby
        .as_ref()
        .map_or(0, |(_, _, _, seats)| seats.len());
    runtime.status = format!(
        "AUTHORITATIVE LAN LAB  [PASS when roster=4]\n\
         real UDP server  {}\n\
         client seat      {player} / {team}\n\
         phase            {phase}\n\
         roster seats     {seats}/4\n\
         reset count      {}\n\
         server thread    owned + stoppable\n\
         R reset server, socket, and client",
        runtime.server.address, runtime.reset_count,
    );
}

fn reset(keyboard: Res<ButtonInput<KeyCode>>, mut runtime: ResMut<LanLabRuntime>) {
    if keyboard.just_pressed(KeyCode::KeyR) {
        let count = runtime.reset_count + 1;
        match LanLabRuntime::start(count) {
            Ok(next) => *runtime = next,
            Err(error) => runtime.status = format!("RESET FAILED: {error}"),
        }
    }
}

fn draw(runtime: Res<LanLabRuntime>, mut text: Query<&mut Text, With<StatusText>>) {
    if let Ok(mut text) = text.single_mut() {
        **text = runtime.status.clone();
    }
}

fn tile_dir() -> PathBuf {
    let cwd = PathBuf::from("assets/tiles");
    if cwd.is_dir() {
        cwd
    } else {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/tiles")
    }
}

pub fn run() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 - LAN Lab".to_string(),
                resolution: (960, 600).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(LanLabPlugin)
        .run();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn production_loopback_session_starts_and_resets_cleanly() {
        let mut first = LanLabRuntime::start(0).expect("first session");
        for _ in 0..100 {
            first.client.poll();
            if first
                .client
                .lobby
                .as_ref()
                .is_some_and(|(_, _, _, seats)| seats.len() == 4)
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(
            first
                .client
                .lobby
                .as_ref()
                .is_some_and(|(_, _, _, seats)| seats.len() == 4)
        );
        drop(first);

        let second = LanLabRuntime::start(1).expect("reset session");
        assert_eq!(second.reset_count, 1);
    }
}
