//! Persistent client/listen-server resources. Presentation lives in `screens`.

use std::net::{Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use bevy::prelude::Resource;
use observed_core::TeamId;
use observed_net::lan::{DEFAULT_LAN_PORT, DiscoveryBrowser, LanClient};
use observed_server::{ServerConfig, ServerHandle};

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
            if let (Some(player), Some((_, _, _, seats))) = (client.player, client.lobby.as_ref())
                && let Some(seat) = seats.iter().find(|seat| seat.player == player)
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

    pub fn host(&mut self) -> Result<(), String> {
        self.leave();
        let mut config = ServerConfig::default();
        config.bind = SocketAddr::from((Ipv4Addr::UNSPECIFIED, DEFAULT_LAN_PORT));
        config.name = "Observed 2 Listen Server".to_string();
        let handle = ServerHandle::spawn(config)?;
        let address = SocketAddr::from((Ipv4Addr::LOCALHOST, handle.address.port()));
        self.listen_server = Some(handle);
        self.connect(address)
    }

    pub fn join_discovered(&mut self) -> Result<(), String> {
        let address = self
            .browser
            .as_ref()
            .and_then(|browser| browser.servers().into_iter().find(|server| server.joinable))
            .map(|server| server.address)
            .ok_or_else(|| "No joinable broadcast server found".to_string())?;
        self.connect(address)
    }

    pub fn join_direct(&mut self) -> Result<(), String> {
        let address = self
            .direct_address
            .parse::<SocketAddr>()
            .map_err(|_| "Enter a valid IP:port".to_string())?;
        self.connect(address)
    }

    fn connect(&mut self, address: SocketAddr) -> Result<(), String> {
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
