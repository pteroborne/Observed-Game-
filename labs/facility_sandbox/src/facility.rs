//! Pure-logic model of the integrated facility: modular rooms, a carryable power
//! source, a deployable structural tool, a powered door, room replacement, and
//! the traversal objective. Geometry is emitted as a `climbing_lab::ClimbWorld`
//! so the proven kinematic + climbing controller drives the players.

use bevy::prelude::*;
use climbing_lab::{ClimbSolid, ClimbWorld, Ladder, LadderId};
use observed_core::{EquipmentId, PlayerId, RoomId};

pub const PLAYER_COUNT: usize = 4;

pub const FLOOR_TOP: f32 = 0.0;
pub const WORLD_MAX_X: f32 = 3000.0;
pub const LADDER_X: f32 = 900.0;
pub const LEDGE_TOP: f32 = 212.0;
pub const DOOR_X: f32 = 1800.0;
pub const DOOR_HEIGHT: f32 = 260.0;
pub const SOCKET_POS: Vec2 = Vec2::new(1600.0, 36.0);
pub const PIT_MIN: f32 = 1980.0;
pub const PIT_MAX: f32 = 2160.0;
pub const BRIDGE_EDGE: Vec2 = Vec2::new(PIT_MIN, 0.0);
pub const GOAL_X: f32 = 2850.0;
pub const REACH: f32 = 70.0;

const BATTERY_START: Vec2 = Vec2::new(LADDER_X, LEDGE_TOP + 34.0);
const JACK_START: Vec2 = Vec2::new(1720.0, 34.0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ItemKind {
    Battery,
    Jack,
}

impl ItemKind {
    pub fn label(self) -> &'static str {
        match self {
            ItemKind::Battery => "POWER CELL",
            ItemKind::Jack => "STRUCTURAL JACK",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ItemLocation {
    Ground(Vec2),
    Carried(PlayerId),
    /// The battery seated in the door's power socket.
    Socketed,
    /// The jack deployed as a bridge over the pit.
    Deployed(Vec2),
}

#[derive(Clone, Copy, Debug)]
pub struct FacilityItem {
    pub id: EquipmentId,
    pub kind: ItemKind,
    pub location: ItemLocation,
}

#[derive(Clone, Copy, Debug)]
pub struct RoomCell {
    pub id: RoomId,
    pub x_min: f32,
    pub x_max: f32,
    /// Selectable authored layout; replacement toggles it.
    pub variant: u8,
}

impl RoomCell {
    pub fn contains(self, x: f32) -> bool {
        x >= self.x_min && x < self.x_max
    }

    pub fn center_x(self) -> f32 {
        (self.x_min + self.x_max) * 0.5
    }
}

#[derive(Resource, Clone, Debug)]
pub struct Facility {
    pub rooms: Vec<RoomCell>,
    pub battery: FacilityItem,
    pub jack: FacilityItem,
    pub door_opened: bool,
    pub pit_bridged: bool,
    pub replacements: u32,
    pub objective_complete: bool,
    pub last_event: String,
}

impl Facility {
    pub fn authored() -> Self {
        let rooms = (0..5)
            .map(|index| RoomCell {
                id: RoomId(index),
                x_min: index as f32 * 600.0,
                x_max: (index as f32 + 1.0) * 600.0,
                variant: 0,
            })
            .collect();
        Self {
            rooms,
            battery: FacilityItem {
                id: EquipmentId(0),
                kind: ItemKind::Battery,
                location: ItemLocation::Ground(BATTERY_START),
            },
            jack: FacilityItem {
                id: EquipmentId(1),
                kind: ItemKind::Jack,
                location: ItemLocation::Ground(JACK_START),
            },
            door_opened: false,
            pit_bridged: false,
            replacements: 0,
            objective_complete: false,
            last_event: "Carry the power cell down from the ledge and feed the door.".to_string(),
        }
    }

    pub fn reset(&mut self) {
        *self = Self::authored();
    }

    pub fn door_open(&self) -> bool {
        matches!(self.battery.location, ItemLocation::Socketed)
    }

    pub fn room_of(&self, x: f32) -> Option<RoomId> {
        self.rooms
            .iter()
            .find(|room| room.contains(x))
            .map(|room| room.id)
    }

    pub fn item_position(&self, item: &FacilityItem, players: &[Vec2]) -> Vec2 {
        match item.location {
            ItemLocation::Ground(position) | ItemLocation::Deployed(position) => position,
            ItemLocation::Socketed => SOCKET_POS,
            ItemLocation::Carried(player) => players
                .get(player.index())
                .copied()
                .map(|p| p + Vec2::new(0.0, 44.0))
                .unwrap_or(SOCKET_POS),
        }
    }

    pub fn carried_by(&self, player: PlayerId) -> Option<ItemKind> {
        for item in [&self.battery, &self.jack] {
            if item.location == ItemLocation::Carried(player) {
                return Some(item.kind);
            }
        }
        None
    }

    /// Build the collision world for the controller from the current state.
    pub fn build_world(&self) -> ClimbWorld {
        let mut solids = Vec::new();

        // Floor in two spans, leaving the pit open until it is bridged.
        push_floor(&mut solids, -40.0, PIT_MIN);
        push_floor(&mut solids, PIT_MAX, WORLD_MAX_X + 40.0);

        // Containing side walls.
        solids.push(wall(-40.0, 600.0));
        solids.push(wall(WORLD_MAX_X + 40.0, 600.0));

        // The climb ledge that holds the power cell.
        solids.push(ClimbSolid {
            center: Vec2::new(LADDER_X, LEDGE_TOP - 12.0),
            half_size: Vec2::new(95.0, 12.0),
        });

        // Powered door: a wall until the cell is socketed.
        if !self.door_open() {
            solids.push(ClimbSolid {
                center: Vec2::new(DOOR_X, DOOR_HEIGHT * 0.5),
                half_size: Vec2::new(16.0, DOOR_HEIGHT * 0.5),
            });
        }

        // Jack bridge over the pit when deployed.
        if matches!(self.jack.location, ItemLocation::Deployed(_)) {
            solids.push(ClimbSolid {
                center: Vec2::new((PIT_MIN + PIT_MAX) * 0.5, -12.0),
                half_size: Vec2::new((PIT_MAX - PIT_MIN) * 0.5 + 12.0, 12.0),
            });
        }

        // Replaceable obstacle in room 0 (variant 1 adds a crate to climb over).
        for room in &self.rooms {
            if room.variant == 1 {
                solids.push(ClimbSolid {
                    center: Vec2::new(room.center_x(), 26.0),
                    half_size: Vec2::new(40.0, 26.0),
                });
            }
        }

        ClimbWorld {
            solids,
            ladders: vec![Ladder {
                id: LadderId(0),
                center_x: LADDER_X,
                half_width: 24.0,
                bottom_y: FLOOR_TOP,
                top_y: LEDGE_TOP,
            }],
            ledges: Vec::new(),
            sockets: Vec::new(),
            bounds_min: Vec2::new(-120.0, -400.0),
            bounds_max: Vec2::new(WORLD_MAX_X + 120.0, 900.0),
        }
    }

    // -- interactions -----------------------------------------------------

    /// Pick up whichever loose item is in reach (battery preferred), if the
    /// player's hands are free.
    pub fn pick_up(&mut self, player: PlayerId, position: Vec2) -> bool {
        if self.carried_by(player).is_some() {
            self.last_event = "Hands full — drop or place first.".to_string();
            return false;
        }
        for kind in [ItemKind::Battery, ItemKind::Jack] {
            let item = self.item_mut(kind);
            if let ItemLocation::Ground(item_position) = item.location
                && item_position.distance(position) <= REACH
            {
                item.location = ItemLocation::Carried(player);
                self.last_event = format!("{} picked up the {}.", player.label(), kind.label());
                return true;
            }
        }
        self.last_event = "Nothing in reach.".to_string();
        false
    }

    pub fn drop_item(&mut self, player: PlayerId, position: Vec2) -> bool {
        let Some(kind) = self.carried_by(player) else {
            return false;
        };
        self.item_mut(kind).location = ItemLocation::Ground(position);
        self.last_event = format!("{} dropped the {}.", player.label(), kind.label());
        true
    }

    /// Context place: socket the battery at the door, or deploy the jack at the
    /// pit, depending on what is carried and where the player stands.
    pub fn place(&mut self, player: PlayerId, position: Vec2) -> bool {
        match self.carried_by(player) {
            Some(ItemKind::Battery) if SOCKET_POS.distance(position) <= REACH => {
                self.battery.location = ItemLocation::Socketed;
                self.door_opened = true;
                self.last_event = "Power cell socketed — the door is open.".to_string();
                true
            }
            Some(ItemKind::Jack) if BRIDGE_EDGE.distance(position) <= REACH => {
                self.jack.location =
                    ItemLocation::Deployed(Vec2::new((PIT_MIN + PIT_MAX) * 0.5, 0.0));
                self.pit_bridged = true;
                self.last_event = "Jack deployed — the pit is bridged.".to_string();
                true
            }
            Some(kind) => {
                self.last_event = format!("No fixture in reach for the {}.", kind.label());
                false
            }
            None => false,
        }
    }

    /// Recover the socketed battery or deployed jack back into the player's hands.
    pub fn recover(&mut self, player: PlayerId, position: Vec2) -> bool {
        if self.carried_by(player).is_some() {
            return false;
        }
        if matches!(self.battery.location, ItemLocation::Socketed)
            && SOCKET_POS.distance(position) <= REACH
        {
            self.battery.location = ItemLocation::Carried(player);
            self.last_event = "Power cell recovered — the door will close.".to_string();
            return true;
        }
        if let ItemLocation::Deployed(bridge) = self.jack.location
            && bridge.distance(position) <= REACH * 2.0
        {
            self.jack.location = ItemLocation::Carried(player);
            self.last_event = "Jack recovered.".to_string();
            return true;
        }
        false
    }

    pub fn replace_room(&mut self, room: RoomId) -> bool {
        let Some(cell) = self.rooms.iter_mut().find(|cell| cell.id == room) else {
            return false;
        };
        cell.variant = (cell.variant + 1) % 2;
        self.replacements += 1;
        self.last_event = format!("Room {} layout replaced.", room.0);
        true
    }

    /// Recompute objective completion from the live positions.
    pub fn update_objective(&mut self, players: &[Vec2]) {
        let players_in_goal =
            players.len() == PLAYER_COUNT && players.iter().all(|p| p.x >= GOAL_X);
        let battery_in_goal = self.item_position(&self.battery, players).x >= GOAL_X;
        self.objective_complete =
            self.door_opened && self.pit_bridged && players_in_goal && battery_in_goal;
    }

    fn item_mut(&mut self, kind: ItemKind) -> &mut FacilityItem {
        match kind {
            ItemKind::Battery => &mut self.battery,
            ItemKind::Jack => &mut self.jack,
        }
    }
}

fn push_floor(solids: &mut Vec<ClimbSolid>, x_min: f32, x_max: f32) {
    solids.push(ClimbSolid {
        center: Vec2::new((x_min + x_max) * 0.5, -20.0),
        half_size: Vec2::new((x_max - x_min) * 0.5, 20.0),
    });
}

fn wall(x: f32, height: f32) -> ClimbSolid {
    ClimbSolid {
        center: Vec2::new(x, height * 0.5),
        half_size: Vec2::new(20.0, height * 0.5),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solids_block_x(world: &ClimbWorld, x: f32, y: f32) -> bool {
        world.solids.iter().any(|solid| {
            (x - solid.center.x).abs() <= solid.half_size.x
                && (y - solid.center.y).abs() <= solid.half_size.y
        })
    }

    #[test]
    fn authored_facility_has_five_rooms_and_loose_tools() {
        let facility = Facility::authored();
        assert_eq!(facility.rooms.len(), 5);
        assert!(matches!(facility.battery.location, ItemLocation::Ground(_)));
        assert!(matches!(facility.jack.location, ItemLocation::Ground(_)));
        assert!(!facility.door_open());
    }

    #[test]
    fn door_blocks_until_the_battery_is_socketed() {
        let mut facility = Facility::authored();
        // Door wall is present in the collision world.
        assert!(solids_block_x(&facility.build_world(), DOOR_X, 100.0));

        // Carry the battery to the socket and place it.
        facility.battery.location = ItemLocation::Carried(PlayerId(0));
        assert!(facility.place(PlayerId(0), SOCKET_POS));
        assert!(facility.door_open());
        assert!(!solids_block_x(&facility.build_world(), DOOR_X, 100.0));
    }

    #[test]
    fn pit_is_open_until_the_jack_is_deployed() {
        let mut facility = Facility::authored();
        let pit_center = (PIT_MIN + PIT_MAX) * 0.5;
        assert!(!solids_block_x(&facility.build_world(), pit_center, -12.0));

        facility.jack.location = ItemLocation::Carried(PlayerId(0));
        assert!(facility.place(PlayerId(0), BRIDGE_EDGE));
        assert!(facility.pit_bridged);
        assert!(solids_block_x(&facility.build_world(), pit_center, -12.0));
    }

    #[test]
    fn pickup_is_proximity_gated_and_single_handed() {
        let mut facility = Facility::authored();
        // Too far from the battery on the ledge.
        assert!(!facility.pick_up(PlayerId(0), Vec2::new(100.0, 34.0)));
        // At the ledge: picks the battery up.
        assert!(facility.pick_up(PlayerId(0), BATTERY_START));
        assert_eq!(facility.carried_by(PlayerId(0)), Some(ItemKind::Battery));
        // Cannot also grab the jack while holding the battery.
        assert!(!facility.pick_up(PlayerId(0), JACK_START));
    }

    #[test]
    fn recovering_the_battery_closes_the_door_again() {
        let mut facility = Facility::authored();
        facility.battery.location = ItemLocation::Carried(PlayerId(0));
        facility.place(PlayerId(0), SOCKET_POS);
        assert!(facility.door_open());

        assert!(facility.recover(PlayerId(0), SOCKET_POS));
        assert_eq!(facility.carried_by(PlayerId(0)), Some(ItemKind::Battery));
        assert!(!facility.door_open());
        // The milestone latch stays set even after recovery.
        assert!(facility.door_opened);
    }

    #[test]
    fn room_replacement_toggles_variant_and_geometry() {
        let mut facility = Facility::authored();
        let before = facility.build_world().solids.len();
        assert!(facility.replace_room(RoomId(0)));
        assert_eq!(facility.replacements, 1);
        assert_eq!(facility.rooms[0].variant, 1);
        assert_eq!(facility.build_world().solids.len(), before + 1);
        // Toggling back removes the obstacle again.
        facility.replace_room(RoomId(0));
        assert_eq!(facility.rooms[0].variant, 0);
        assert_eq!(facility.build_world().solids.len(), before);
    }

    #[test]
    fn objective_completes_only_after_every_milestone() {
        let mut facility = Facility::authored();
        let at_goal = [Vec2::new(GOAL_X + 20.0, 34.0); PLAYER_COUNT];

        // Players at the goal but no power/bridge yet.
        facility.update_objective(&at_goal);
        assert!(!facility.objective_complete);

        // Power the door and bridge the pit.
        facility.door_opened = true;
        facility.pit_bridged = true;
        // Battery carried by a player who is at the goal.
        facility.battery.location = ItemLocation::Carried(PlayerId(0));
        facility.update_objective(&at_goal);
        assert!(facility.objective_complete);

        // If one player lags behind, it is no longer complete.
        let mut split = at_goal;
        split[2] = Vec2::new(1000.0, 34.0);
        facility.update_objective(&split);
        assert!(!facility.objective_complete);
    }
}
