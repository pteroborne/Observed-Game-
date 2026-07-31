//! Persistent client/listen-server resources. Presentation lives in `screens`.

use std::net::{Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use bevy::prelude::Resource;
use observed_core::TeamId;
use observed_net::lan::{DEFAULT_LAN_PORT, DiscoveryBrowser, LanClient};
use observed_server::{ServerConfig, ServerHandle};

use crate::play_setup::ValidatedPlaySetup;

#[derive(Resource)]
pub(crate) struct LanRuntime {
    pub browser: Option<DiscoveryBrowser>,
    pub client: Option<LanClient>,
    pub listen_server: Option<ServerHandle>,
    pub direct_address: String,
    pub status: String,
    pub ready: bool,
    pub consumed_match: Option<u32>,
    pub requested_team: Option<TeamId>,
    pub last_probe: Instant,
    account: u16,
}

impl LanRuntime {
    pub fn new() -> Self {
        let direct_address = std::env::var("OBSERVED2_LAN_ADDRESS")
            .unwrap_or_else(|_| format!("127.0.0.1:{DEFAULT_LAN_PORT}"));
        Self {
            browser: DiscoveryBrowser::bind(DEFAULT_LAN_PORT).ok(),
            client: None,
            listen_server: None,
            direct_address,
            status: "Search the LAN or enter an address.".to_string(),
            ready: false,
            consumed_match: None,
            requested_team: None,
            last_probe: Instant::now() - Duration::from_secs(2),
            account: local_account(),
        }
    }

    pub fn poll(&mut self) {
        if let Some(browser) = self.browser.as_mut() {
            if self.last_probe.elapsed() >= Duration::from_secs(1) {
                let _ = browser.probe();
                self.last_probe = Instant::now();
            }
            browser.poll();
        }
        if let Some(client) = self.client.as_mut() {
            client.poll();
            if let (Some(player), Some(lobby)) = (client.player, client.lobby.as_ref())
                && let Some(seat) = lobby.seats.iter().find(|seat| seat.player == player)
            {
                self.ready = seat.ready;
            }
            if let Some(reason) = client.rejection.clone() {
                self.status = reason;
            } else if let (Some(player), Some(team)) = (client.player, client.team) {
                self.status = format!("Connected as {} / {}", player.label(), team.label());
            }
        }
    }

    /// Start a listen server with the roster and Guardian rule already chosen in
    /// the Play Hub, then connect the local player to its exact bound address.
    pub fn host_with_setup(&mut self, setup: ValidatedPlaySetup) -> Result<(), String> {
        self.leave();
        let mut config = listen_server_config(setup)?;
        config.bind = SocketAddr::from((Ipv4Addr::UNSPECIFIED, DEFAULT_LAN_PORT));
        config.name = "Observed 2 Listen Server".to_string();
        let handle = ServerHandle::spawn(config)?;
        let address = SocketAddr::from((Ipv4Addr::LOCALHOST, handle.address.port()));
        self.listen_server = Some(handle);
        self.join_server(address)
    }

    /// Connect to the exact address selected by the browser screen.
    ///
    /// Keeping address selection outside this transport resource prevents discovery
    /// refresh order from silently changing which server a visible Join action targets.
    pub fn join_server(&mut self, address: SocketAddr) -> Result<(), String> {
        let resume = self.client.as_ref().and_then(|client| client.token);
        self.client = Some(
            LanClient::connect(
                address,
                self.account,
                self.requested_team,
                resume,
                crate::hex_wfc::sim::simulation_content_hash(),
            )
            .map_err(|error| format!("connect {address}: {error}"))?,
        );
        self.ready = false;
        self.status = format!("Connecting to {address}...");
        Ok(())
    }

    pub fn toggle_ready(&mut self) -> Result<(), String> {
        self.ready = !self.ready;
        self.client
            .as_ref()
            .ok_or_else(|| "not connected".to_string())?
            .set_ready(self.ready)
            .map_err(|error| error.to_string())
    }

    pub fn request_team(&mut self, team: TeamId) -> Result<(), String> {
        self.requested_team = Some(team);
        self.client
            .as_ref()
            .ok_or_else(|| "not connected".to_string())?
            .request_team(team)
            .map_err(|error| error.to_string())
    }

    pub fn leave(&mut self) {
        if let Some(client) = self.client.take() {
            client.goodbye();
        }
        if let Some(mut server) = self.listen_server.take() {
            let _ = server.shutdown();
        }
        self.ready = false;
        self.consumed_match = None;
        self.status = "Disconnected.".to_string();
    }
}

fn local_account() -> u16 {
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.subsec_nanos());
    ((std::process::id() ^ time) & u32::from(u16::MAX)) as u16
}

fn listen_server_config(setup: ValidatedPlaySetup) -> Result<ServerConfig, String> {
    let mut arguments = vec![
        "observed-listen-server".to_string(),
        "--teams".to_string(),
        setup.teams.to_string(),
        "--team-size".to_string(),
        setup.members_per_team.to_string(),
    ];
    if !setup.guardian {
        arguments.push("--no-guardian".to_string());
    }
    if !setup.fill_empty_seats {
        arguments.push("--require-full-roster".to_string());
    }
    ServerConfig::from_args(arguments)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::play_setup::{PlayPreset, PlaySetupDraft};

    #[test]
    fn join_server_reports_the_exact_selected_address() {
        let mut lan = LanRuntime::new();
        let address = SocketAddr::from((Ipv4Addr::LOCALHOST, 47_625));
        lan.join_server(address)
            .expect("a nonblocking loopback UDP client can bind");
        assert_eq!(lan.status, format!("Connecting to {address}..."));
        assert!(lan.client.is_some());
    }

    #[test]
    fn listen_host_uses_the_validated_play_hub_roster() {
        let setup = PlaySetupDraft {
            preset: PlayPreset::Custom,
            teams: 4,
            members_per_team: 4,
            fill_empty_seats: true,
            guardian: false,
        }
        .validate()
        .expect("4x4 is a valid LAN roster");
        let config = listen_server_config(setup).expect("validated setup builds server config");
        assert_eq!(config.roster.teams, 4);
        assert_eq!(config.roster.members_per_team, 4);
        assert!(config.fill_empty_seats);
    }

    #[test]
    fn humans_only_listen_host_requires_the_complete_roster() {
        let setup = PlaySetupDraft {
            preset: PlayPreset::Custom,
            teams: 2,
            members_per_team: 3,
            fill_empty_seats: false,
            guardian: true,
        }
        .validate()
        .expect("2x3 is a valid LAN roster");
        let config = listen_server_config(setup).expect("validated setup builds server config");
        assert!(!config.fill_empty_seats);
    }
}
