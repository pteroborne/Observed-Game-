use std::collections::BTreeMap;

use bevy::prelude::*;
use observed_core::{PlayerId, TeamId};

pub const MOVE_SPEED: f32 = 230.0;
pub const ARENA_MIN: Vec2 = Vec2::new(-560.0, -200.0);
pub const ARENA_MAX: Vec2 = Vec2::new(560.0, 200.0);
const MACHINE_RATE: f32 = 0.5;
const MACHINE_DECAY: f32 = 1.0;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StationId(pub u16);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ItemId(pub u16);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ZoneId(pub u8);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StationKind {
    /// Capacity-1 chokepoint. Only one player may pass at a time.
    NarrowPassage,
    /// Multi-occupant climb surface. Several players climb at once.
    ClimbPoint,
    /// Cooperative machine. Makes progress only while enough operators occupy it.
    Machine,
}

impl StationKind {
    pub fn label(self) -> &'static str {
        match self {
            StationKind::NarrowPassage => "NARROW PASSAGE",
            StationKind::ClimbPoint => "CLIMB POINT",
            StationKind::Machine => "MACHINE",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Station {
    pub id: StationId,
    pub kind: StationKind,
    pub position: Vec2,
    pub radius: f32,
    pub capacity: usize,
    /// Operators required before a `Machine` makes progress.
    pub required: usize,
    pub occupants: Vec<PlayerId>,
    pub progress: f32,
    pub activations: u32,
}

impl Station {
    pub fn in_range(&self, point: Vec2) -> bool {
        self.position.distance(point) <= self.radius
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Item {
    pub id: ItemId,
    pub position: Vec2,
    pub radius: f32,
    pub holder: Option<PlayerId>,
}

#[derive(Clone, Copy, Debug)]
pub struct Zone {
    pub id: ZoneId,
    pub name: &'static str,
    pub center: Vec2,
    pub half_size: Vec2,
}

impl Zone {
    pub fn contains(self, point: Vec2) -> bool {
        (point.x - self.center.x).abs() <= self.half_size.x
            && (point.y - self.center.y).abs() <= self.half_size.y
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TeamPlayer {
    pub id: PlayerId,
    pub team: TeamId,
    pub position: Vec2,
    pub spawn_position: Vec2,
    pub occupying: Option<StationId>,
    pub carrying: Option<ItemId>,
}

/// One tick of intent for a single player. Produced by human input or a bot;
/// the simulation never inspects where it came from.
#[derive(Clone, Copy, Debug, Default)]
pub struct TeamIntent {
    pub movement: Vec2,
    pub use_station: Option<StationId>,
    pub release_station: bool,
    pub grab_item: Option<ItemId>,
    pub drop_item: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TeamEvent {
    EnteredStation {
        player: PlayerId,
        station: StationId,
    },
    LeftStation {
        player: PlayerId,
        station: StationId,
    },
    GrabbedItem {
        player: PlayerId,
        item: ItemId,
    },
    DroppedItem {
        player: PlayerId,
        item: ItemId,
    },
    MachineActivated {
        station: StationId,
    },
    Reunited {
        team: TeamId,
    },
    Separated {
        team: TeamId,
    },
    Denied {
        player: PlayerId,
        reason: &'static str,
    },
}

impl TeamEvent {
    pub fn label(self) -> String {
        match self {
            TeamEvent::EnteredStation { player, station } => {
                format!("{} entered station {}.", player.label(), station.0)
            }
            TeamEvent::LeftStation { player, station } => {
                format!("{} left station {}.", player.label(), station.0)
            }
            TeamEvent::GrabbedItem { player, item } => {
                format!("{} grabbed item {}.", player.label(), item.0)
            }
            TeamEvent::DroppedItem { player, item } => {
                format!("{} dropped item {}.", player.label(), item.0)
            }
            TeamEvent::MachineActivated { station } => {
                format!("Machine {} completed a cycle.", station.0)
            }
            TeamEvent::Reunited { team } => format!("{} reunited.", team.label()),
            TeamEvent::Separated { team } => format!("{} separated.", team.label()),
            TeamEvent::Denied { player, reason } => {
                format!("{} denied: {reason}.", player.label())
            }
        }
    }
}

#[derive(Resource, Clone, Debug)]
pub struct TeamWorld {
    pub teams: Vec<TeamId>,
    pub players: Vec<TeamPlayer>,
    pub stations: Vec<Station>,
    pub items: Vec<Item>,
    pub zones: Vec<Zone>,
    pub team_cohesive: Vec<bool>,
    pub reunions: u32,
    pub separations: u32,
    pub denials: u32,
    pub recent_events: Vec<TeamEvent>,
    pub total_events: u32,
}

impl TeamWorld {
    pub fn authored_lab() -> Self {
        let team_a = TeamId(0);
        let team_b = TeamId(1);
        let zones = vec![
            Zone {
                id: ZoneId(0),
                name: "WEST",
                center: Vec2::new(-360.0, 0.0),
                half_size: Vec2::new(180.0, 195.0),
            },
            Zone {
                id: ZoneId(1),
                name: "RALLY",
                center: Vec2::new(0.0, 0.0),
                half_size: Vec2::new(140.0, 195.0),
            },
            Zone {
                id: ZoneId(2),
                name: "EAST",
                center: Vec2::new(360.0, 0.0),
                half_size: Vec2::new(180.0, 195.0),
            },
        ];
        let stations = vec![
            Station {
                id: StationId(0),
                kind: StationKind::NarrowPassage,
                position: Vec2::new(-160.0, 0.0),
                radius: 64.0,
                capacity: 1,
                required: 1,
                occupants: vec![PlayerId(1)],
                progress: 0.0,
                activations: 0,
            },
            Station {
                id: StationId(1),
                kind: StationKind::NarrowPassage,
                position: Vec2::new(160.0, 0.0),
                radius: 64.0,
                capacity: 1,
                required: 1,
                occupants: Vec::new(),
                progress: 0.0,
                activations: 0,
            },
            Station {
                id: StationId(2),
                kind: StationKind::ClimbPoint,
                position: Vec2::new(-380.0, 80.0),
                radius: 74.0,
                capacity: 3,
                required: 1,
                occupants: vec![PlayerId(0)],
                progress: 0.0,
                activations: 0,
            },
            Station {
                id: StationId(3),
                kind: StationKind::Machine,
                position: Vec2::new(380.0, -40.0),
                radius: 82.0,
                capacity: 2,
                required: 2,
                occupants: vec![PlayerId(2), PlayerId(3)],
                progress: 0.0,
                activations: 0,
            },
        ];
        let items = vec![
            Item {
                id: ItemId(0),
                position: Vec2::new(-300.0, -110.0),
                radius: 42.0,
                holder: None,
            },
            Item {
                id: ItemId(1),
                position: Vec2::new(300.0, 110.0),
                radius: 42.0,
                holder: None,
            },
        ];
        let players = vec![
            TeamPlayer {
                id: PlayerId(0),
                team: team_a,
                position: Vec2::new(-380.0, 100.0),
                spawn_position: Vec2::new(-380.0, 100.0),
                occupying: Some(StationId(2)),
                carrying: None,
            },
            TeamPlayer {
                id: PlayerId(1),
                team: team_a,
                position: Vec2::new(-160.0, 0.0),
                spawn_position: Vec2::new(-160.0, 0.0),
                occupying: Some(StationId(0)),
                carrying: None,
            },
            TeamPlayer {
                id: PlayerId(2),
                team: team_b,
                position: Vec2::new(360.0, -40.0),
                spawn_position: Vec2::new(360.0, -40.0),
                occupying: Some(StationId(3)),
                carrying: None,
            },
            TeamPlayer {
                id: PlayerId(3),
                team: team_b,
                position: Vec2::new(404.0, -40.0),
                spawn_position: Vec2::new(404.0, -40.0),
                occupying: Some(StationId(3)),
                carrying: None,
            },
        ];

        let mut world = Self {
            teams: vec![team_a, team_b],
            players,
            stations,
            items,
            zones,
            team_cohesive: vec![false, false],
            reunions: 0,
            separations: 0,
            denials: 0,
            recent_events: Vec::new(),
            total_events: 0,
        };
        // Seed cohesion from the authored positions without counting it as a
        // reunion on the first tick.
        world.team_cohesive = world
            .teams
            .iter()
            .map(|team| world.team_together(*team))
            .collect();
        world
    }

    pub fn reset(&mut self) {
        *self = Self::authored_lab();
    }

    // -- lookups ----------------------------------------------------------

    pub fn player(&self, id: PlayerId) -> Option<&TeamPlayer> {
        self.players.iter().find(|player| player.id == id)
    }

    pub fn station(&self, id: StationId) -> Option<&Station> {
        self.stations.iter().find(|station| station.id == id)
    }

    pub fn item(&self, id: ItemId) -> Option<&Item> {
        self.items.iter().find(|item| item.id == id)
    }

    pub fn zone_of(&self, point: Vec2) -> Option<ZoneId> {
        self.zones
            .iter()
            .find(|zone| zone.contains(point))
            .map(|zone| zone.id)
    }

    pub fn team_members(&self, team: TeamId) -> impl Iterator<Item = &TeamPlayer> {
        self.players
            .iter()
            .filter(move |player| player.team == team)
    }

    pub fn team_together(&self, team: TeamId) -> bool {
        let zones: Vec<Option<ZoneId>> = self
            .team_members(team)
            .map(|player| self.zone_of(player.position))
            .collect();
        !zones.is_empty() && zones[0].is_some() && zones.iter().all(|zone| *zone == zones[0])
    }

    pub fn cohesive(&self, team: TeamId) -> bool {
        self.teams
            .iter()
            .position(|candidate| *candidate == team)
            .map(|index| self.team_cohesive[index])
            .unwrap_or(false)
    }

    fn player_index(&self, id: PlayerId) -> Option<usize> {
        self.players.iter().position(|player| player.id == id)
    }

    fn station_index(&self, id: StationId) -> Option<usize> {
        self.stations.iter().position(|station| station.id == id)
    }

    fn item_index(&self, id: ItemId) -> Option<usize> {
        self.items.iter().position(|item| item.id == id)
    }

    // -- simulation -------------------------------------------------------

    /// Advance the world by one tick from a set of per-player intents. Requests
    /// are always resolved in ascending `PlayerId` order, so contention is
    /// deterministic regardless of the order intents arrive in.
    pub fn tick(&mut self, intents: &[(PlayerId, TeamIntent)], dt: f32) {
        let map: BTreeMap<PlayerId, TeamIntent> = intents.iter().copied().collect();

        for player in &mut self.players {
            if let Some(intent) = map.get(&player.id) {
                player.position = (player.position + intent.movement * MOVE_SPEED * dt)
                    .clamp(ARENA_MIN, ARENA_MAX);
            }
        }

        self.process_station_releases(&map);
        self.process_station_requests(&map);
        self.process_item_changes(&map);
        self.advance_machines(dt);
        self.update_cohesion();
    }

    fn process_station_releases(&mut self, map: &BTreeMap<PlayerId, TeamIntent>) {
        let mut releases: Vec<(PlayerId, StationId)> = Vec::new();
        for player in &self.players {
            if let Some(station_id) = player.occupying {
                let asked = map
                    .get(&player.id)
                    .is_some_and(|intent| intent.release_station);
                let out_of_range = self
                    .station(station_id)
                    .is_none_or(|station| !station.in_range(player.position));
                if asked || out_of_range {
                    releases.push((player.id, station_id));
                }
            }
        }
        for (player, station) in releases {
            if let Some(index) = self.player_index(player) {
                self.players[index].occupying = None;
            }
            if let Some(index) = self.station_index(station) {
                self.stations[index].occupants.retain(|id| *id != player);
            }
            self.push_event(TeamEvent::LeftStation { player, station });
        }
    }

    fn process_station_requests(&mut self, map: &BTreeMap<PlayerId, TeamIntent>) {
        let mut requests: Vec<(PlayerId, StationId)> = Vec::new();
        for player in &self.players {
            if player.occupying.is_some() {
                continue;
            }
            let Some(station_id) = map.get(&player.id).and_then(|intent| intent.use_station) else {
                continue;
            };
            if self
                .station(station_id)
                .is_some_and(|station| station.in_range(player.position))
            {
                requests.push((player.id, station_id));
            }
        }
        // Deterministic admission: lowest PlayerId first.
        requests.sort_by_key(|(player, _)| player.0);
        for (player, station_id) in requests {
            let index = self.station_index(station_id).expect("station exists");
            if self.stations[index].occupants.len() < self.stations[index].capacity {
                self.stations[index].occupants.push(player);
                self.stations[index].occupants.sort_by_key(|id| id.0);
                if let Some(player_index) = self.player_index(player) {
                    self.players[player_index].occupying = Some(station_id);
                }
                self.push_event(TeamEvent::EnteredStation {
                    player,
                    station: station_id,
                });
            } else {
                self.denials += 1;
                self.push_event(TeamEvent::Denied {
                    player,
                    reason: "station at capacity",
                });
            }
        }
    }

    fn process_item_changes(&mut self, map: &BTreeMap<PlayerId, TeamIntent>) {
        // Drops first, freeing items for grabs in the same tick.
        let mut drops: Vec<PlayerId> = Vec::new();
        for player in &self.players {
            if player.carrying.is_some() && map.get(&player.id).is_some_and(|i| i.drop_item) {
                drops.push(player.id);
            }
        }
        for player in drops {
            let index = self.player_index(player).unwrap();
            if let Some(item) = self.players[index].carrying.take()
                && let Some(item_index) = self.item_index(item)
            {
                self.items[item_index].holder = None;
                self.items[item_index].position = self.players[index].position;
                self.push_event(TeamEvent::DroppedItem { player, item });
            }
        }

        let mut requests: Vec<(PlayerId, ItemId)> = Vec::new();
        for player in &self.players {
            if player.carrying.is_some() {
                continue;
            }
            let Some(item_id) = map.get(&player.id).and_then(|intent| intent.grab_item) else {
                continue;
            };
            if self
                .item(item_id)
                .is_some_and(|item| item.position.distance(player.position) <= item.radius)
            {
                requests.push((player.id, item_id));
            }
        }
        requests.sort_by_key(|(player, _)| player.0);
        for (player, item_id) in requests {
            let item_index = self.item_index(item_id).expect("item exists");
            let player_index = self.player_index(player).expect("player exists");
            if self.items[item_index].holder.is_none()
                && self.players[player_index].carrying.is_none()
            {
                self.items[item_index].holder = Some(player);
                self.players[player_index].carrying = Some(item_id);
                self.push_event(TeamEvent::GrabbedItem {
                    player,
                    item: item_id,
                });
            } else {
                self.denials += 1;
                self.push_event(TeamEvent::Denied {
                    player,
                    reason: "item already taken",
                });
            }
        }
    }

    fn advance_machines(&mut self, dt: f32) {
        let mut activated: Vec<StationId> = Vec::new();
        for station in &mut self.stations {
            if station.kind != StationKind::Machine {
                continue;
            }
            if station.occupants.len() >= station.required {
                station.progress += MACHINE_RATE * dt;
                if station.progress >= 1.0 {
                    station.progress = 0.0;
                    station.activations += 1;
                    activated.push(station.id);
                }
            } else {
                station.progress = (station.progress - MACHINE_DECAY * dt).max(0.0);
            }
        }
        for station in activated {
            self.push_event(TeamEvent::MachineActivated { station });
        }
    }

    fn update_cohesion(&mut self) {
        let mut events: Vec<TeamEvent> = Vec::new();
        let teams = self.teams.clone();
        for (index, team) in teams.iter().enumerate() {
            let cohesive = self.team_together(*team);
            let was = self.team_cohesive[index];
            if cohesive && !was {
                self.reunions += 1;
                events.push(TeamEvent::Reunited { team: *team });
            } else if !cohesive && was {
                self.separations += 1;
                events.push(TeamEvent::Separated { team: *team });
            }
            self.team_cohesive[index] = cohesive;
        }
        for event in events {
            self.push_event(event);
        }
    }

    fn push_event(&mut self, event: TeamEvent) {
        self.total_events += 1;
        self.recent_events.push(event);
        if self.recent_events.len() > 8 {
            self.recent_events.remove(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    fn empty_world() -> TeamWorld {
        // A minimal world with one of each contested resource, no preset occupancy.
        TeamWorld {
            teams: vec![TeamId(0), TeamId(1)],
            players: vec![
                player(0, 0, Vec2::new(0.0, 0.0)),
                player(1, 0, Vec2::new(0.0, 0.0)),
                player(2, 1, Vec2::new(0.0, 0.0)),
                player(3, 1, Vec2::new(0.0, 0.0)),
            ],
            stations: vec![
                station(0, StationKind::NarrowPassage, Vec2::ZERO, 1, 1),
                station(1, StationKind::ClimbPoint, Vec2::ZERO, 3, 1),
                station(2, StationKind::Machine, Vec2::ZERO, 2, 2),
            ],
            items: vec![Item {
                id: ItemId(0),
                position: Vec2::ZERO,
                radius: 50.0,
                holder: None,
            }],
            zones: vec![
                Zone {
                    id: ZoneId(0),
                    name: "LEFT",
                    center: Vec2::new(-200.0, 0.0),
                    half_size: Vec2::new(100.0, 200.0),
                },
                Zone {
                    id: ZoneId(1),
                    name: "RIGHT",
                    center: Vec2::new(200.0, 0.0),
                    half_size: Vec2::new(100.0, 200.0),
                },
            ],
            team_cohesive: vec![false, false],
            reunions: 0,
            separations: 0,
            denials: 0,
            recent_events: Vec::new(),
            total_events: 0,
        }
    }

    fn player(id: u16, team: u8, position: Vec2) -> TeamPlayer {
        TeamPlayer {
            id: PlayerId(id),
            team: TeamId(team),
            position,
            spawn_position: position,
            occupying: None,
            carrying: None,
        }
    }

    fn station(
        id: u16,
        kind: StationKind,
        position: Vec2,
        capacity: usize,
        required: usize,
    ) -> Station {
        Station {
            id: StationId(id),
            kind,
            position,
            radius: 80.0,
            capacity,
            required,
            occupants: Vec::new(),
            progress: 0.0,
            activations: 0,
        }
    }

    fn use_station(station: u16) -> TeamIntent {
        TeamIntent {
            use_station: Some(StationId(station)),
            ..default()
        }
    }

    #[test]
    fn authored_lab_has_two_teams_of_two() {
        let world = TeamWorld::authored_lab();
        assert_eq!(world.players.len(), 4);
        assert_eq!(world.teams.len(), 2);
        assert_eq!(world.team_members(TeamId(0)).count(), 2);
        assert_eq!(world.team_members(TeamId(1)).count(), 2);
        // Team B starts together at the machine; team A is split (passage + climb).
        assert!(world.cohesive(TeamId(1)));
        assert!(!world.cohesive(TeamId(0)));
    }

    #[test]
    fn narrow_passage_admits_one_and_denies_the_rest_deterministically() {
        let mut world = empty_world();
        // P2 and P3 both reach for the capacity-1 passage in the same tick.
        world.tick(
            &[(PlayerId(3), use_station(0)), (PlayerId(2), use_station(0))],
            DT,
        );
        assert_eq!(
            world.station(StationId(0)).unwrap().occupants,
            vec![PlayerId(2)]
        );
        assert_eq!(
            world.player(PlayerId(2)).unwrap().occupying,
            Some(StationId(0))
        );
        assert_eq!(world.player(PlayerId(3)).unwrap().occupying, None);
        assert_eq!(world.denials, 1);
    }

    #[test]
    fn input_order_does_not_change_contention_outcome() {
        let mut a = empty_world();
        let mut b = empty_world();
        a.tick(
            &[(PlayerId(0), use_station(0)), (PlayerId(1), use_station(0))],
            DT,
        );
        b.tick(
            &[(PlayerId(1), use_station(0)), (PlayerId(0), use_station(0))],
            DT,
        );
        assert_eq!(
            a.station(StationId(0)).unwrap().occupants,
            b.station(StationId(0)).unwrap().occupants
        );
        assert_eq!(
            a.station(StationId(0)).unwrap().occupants,
            vec![PlayerId(0)]
        );
    }

    #[test]
    fn climb_point_holds_multiple_climbers_at_once() {
        let mut world = empty_world();
        world.tick(
            &[
                (PlayerId(0), use_station(1)),
                (PlayerId(1), use_station(1)),
                (PlayerId(2), use_station(1)),
            ],
            DT,
        );
        assert_eq!(world.station(StationId(1)).unwrap().occupants.len(), 3);
    }

    #[test]
    fn machine_needs_two_operators_and_activates() {
        let mut world = empty_world();
        // One operator: no progress.
        world.tick(&[(PlayerId(0), use_station(2))], DT);
        world.tick(&[], DT);
        assert_eq!(world.station(StationId(2)).unwrap().progress, 0.0);

        // Second operator joins: progress accrues and eventually activates.
        let mut activated = false;
        for _ in 0..600 {
            world.tick(&[(PlayerId(1), use_station(2))], DT);
            if world.station(StationId(2)).unwrap().activations > 0 {
                activated = true;
                break;
            }
        }
        assert!(activated);
        assert_eq!(world.station(StationId(2)).unwrap().occupants.len(), 2);
    }

    #[test]
    fn item_contention_gives_it_to_the_lowest_player_id() {
        let mut world = empty_world();
        world.tick(
            &[
                (
                    PlayerId(2),
                    TeamIntent {
                        grab_item: Some(ItemId(0)),
                        ..default()
                    },
                ),
                (
                    PlayerId(1),
                    TeamIntent {
                        grab_item: Some(ItemId(0)),
                        ..default()
                    },
                ),
            ],
            DT,
        );
        assert_eq!(world.item(ItemId(0)).unwrap().holder, Some(PlayerId(1)));
        assert_eq!(world.player(PlayerId(1)).unwrap().carrying, Some(ItemId(0)));
        assert_eq!(world.player(PlayerId(2)).unwrap().carrying, None);
    }

    #[test]
    fn leaving_range_auto_releases_a_station_for_the_next_player() {
        let mut world = empty_world();
        world.tick(&[(PlayerId(0), use_station(0))], DT);
        assert_eq!(
            world.player(PlayerId(0)).unwrap().occupying,
            Some(StationId(0))
        );

        // Walk P0 far away; the passage frees up.
        world.tick(
            &[(
                PlayerId(0),
                TeamIntent {
                    movement: Vec2::new(0.0, 1.0),
                    ..default()
                },
            )],
            10.0,
        );
        assert_eq!(world.player(PlayerId(0)).unwrap().occupying, None);
        assert!(world.station(StationId(0)).unwrap().occupants.is_empty());

        // Now P1 can take it.
        world.tick(&[(PlayerId(1), use_station(0))], DT);
        assert_eq!(
            world.station(StationId(0)).unwrap().occupants,
            vec![PlayerId(1)]
        );
    }

    #[test]
    fn team_separates_and_reunites_across_zones() {
        let mut world = empty_world();
        // Put both of team A in the LEFT zone -> reunited.
        place(&mut world, PlayerId(0), Vec2::new(-200.0, 0.0));
        place(&mut world, PlayerId(1), Vec2::new(-200.0, 50.0));
        world.tick(&[], DT);
        assert!(world.cohesive(TeamId(0)));
        assert_eq!(world.reunions, 1);

        // Send P1 to the RIGHT zone -> separated.
        place(&mut world, PlayerId(1), Vec2::new(200.0, 0.0));
        world.tick(&[], DT);
        assert!(!world.cohesive(TeamId(0)));
        assert_eq!(world.separations, 1);

        // Bring P1 back -> reunited again.
        place(&mut world, PlayerId(1), Vec2::new(-200.0, -30.0));
        world.tick(&[], DT);
        assert!(world.cohesive(TeamId(0)));
        assert_eq!(world.reunions, 2);
    }

    #[test]
    fn reset_restores_the_authored_baseline() {
        let mut world = TeamWorld::authored_lab();
        world.tick(&[(PlayerId(0), use_station(2))], DT);
        world.denials += 5;
        world.reset();
        assert_eq!(world.players.len(), 4);
        assert_eq!(world.denials, 0);
        assert!(world.cohesive(TeamId(1)));
        assert_eq!(world.station(StationId(3)).unwrap().occupants.len(), 2);
    }

    fn place(world: &mut TeamWorld, player: PlayerId, position: Vec2) {
        let index = world.player_index(player).unwrap();
        world.players[index].position = position;
        world.players[index].spawn_position = position;
    }
}
