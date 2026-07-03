//! Pure-logic feasibility model for **Betrayal-style room discovery + a gated exit**.
//!
//! The facility's rooms have a hidden **type** ([`RoomType`]). You only learn a room's
//! type by **visiting** it, and unobserved rooms **shift** their types when you look away
//! — so the room you remember as a keystone vault may be a dead end when you come back.
//! The exit is **gated**: it stays locked until the team has collected the required
//! **keystones** (from Keystone Vaults) and **power** (from Power Caches / Reactors).
//!
//! The vocabulary started at a "core 5" (Keystone Vault, Power Cache, Control, Survey,
//! Dead-end) and is expanded here with three more, each posing its own behavioural
//! question:
//! - **Reactor** — a richer power node (yields 2), so the power economy is *yield-based*,
//!   not a simple count.
//! - **Sensor** — reveals types *at range* (adjacent rooms only), distinct from Survey's
//!   facility-wide reveal.
//! - **Decoy** — the deepest Betrayal: it **lies**, *displaying* as a Keystone Vault when
//!   revealed remotely, but yielding nothing when actually visited. The lie misleads the
//!   human; it is never counted as a real keystone, so it cannot affect solvability.
//!
//! The primary technical question — the analogue of `constraint_lab`'s protected spine —
//! is **solvability**: a single rule (only shift types among *unharvested* rooms) keeps
//! the objective *always completable*, because keystones can never strand on a spent
//! room. With that constraint off, the same shifting can strand a keystone on an
//! already-harvested room and make the objective impossible — which is exactly why the
//! constraint exists. Everything here is deterministic.

use observed_core::SplitMix;
use observed_style::DoorIdentityRole;

use bevy::prelude::*;

/// The room types. Each room is exactly one of these at any moment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomType {
    /// Yields a **keystone** when harvested (the key the gate needs).
    KeystoneVault,
    /// Yields **power** when harvested (a gate requirement).
    PowerCache,
    /// A richer power node — yields **2** power.
    Reactor,
    /// Stabilises the facility: types stop shifting once it is harvested.
    Control,
    /// Reveals every room's *current* type (a snapshot that decays as things shift).
    Survey,
    /// Reveals only **adjacent** rooms' current types — a survey *at range*.
    Sensor,
    /// Lies: displays as a Keystone Vault when revealed remotely, but yields nothing on a
    /// visit. Never counted as a real keystone, so it can mislead but never strand the run.
    Decoy,
    /// Nothing — the bust (the "Betrayal" disappointment).
    DeadEnd,
}

impl RoomType {
    pub fn label(self) -> &'static str {
        match self {
            RoomType::KeystoneVault => "Keystone Vault",
            RoomType::PowerCache => "Power Cache",
            RoomType::Reactor => "Reactor",
            RoomType::Control => "Control",
            RoomType::Survey => "Survey",
            RoomType::Sensor => "Sensor",
            RoomType::Decoy => "Decoy",
            RoomType::DeadEnd => "Dead-end",
        }
    }

    /// A single glyph for the schematic.
    pub fn glyph(self) -> char {
        match self {
            RoomType::KeystoneVault => 'K',
            RoomType::PowerCache => 'P',
            RoomType::Reactor => 'R',
            RoomType::Control => 'C',
            RoomType::Survey => 'S',
            RoomType::Sensor => 'N',
            RoomType::Decoy => '!',
            RoomType::DeadEnd => '.',
        }
    }

    /// Power produced when this room is harvested (0 for non-power rooms).
    pub fn power_yield(self) -> u32 {
        match self {
            RoomType::PowerCache => 1,
            RoomType::Reactor => 2,
            _ => 0,
        }
    }
}

/// Facility grid dimensions (a 3×3 schematic). Adjacency for [`RoomType::Sensor`] is the
/// 4-neighbourhood on this grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KnowledgeSource {
    Visit,
    Survey,
    Sensor,
}

impl KnowledgeSource {
    pub fn label(self) -> &'static str {
        match self {
            KnowledgeSource::Visit => "visit",
            KnowledgeSource::Survey => "survey",
            KnowledgeSource::Sensor => "sensor",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BotStrategy {
    Reader,
    Random,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BotRunOutcome {
    pub strategy: BotStrategy,
    pub seed: u64,
    pub escaped: bool,
    pub visits: u32,
    pub shifts: u32,
    pub wasted_visits: u32,
    pub sensor_map_updates: u32,
    pub decoy_lies_resolved: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BotComparison {
    pub seeds: u32,
    pub reader_escapes: u32,
    pub random_escapes: u32,
    pub reader_mean_visits: f32,
    pub random_mean_visits: f32,
    pub visit_margin: f32,
}

pub const GRID_COLS: usize = 3;
pub const GRID_ROWS: usize = 3;
/// Number of rooms in the lab facility.
pub const ROOM_COUNT: usize = GRID_COLS * GRID_ROWS;

/// The authored multiset of types: 2 Keystone Vaults, 1 Power Cache, 1 Reactor, 1 Control,
/// 1 Survey, 1 Sensor, 1 Decoy, 1 Dead-end. Decoherence only ever **permutes** this
/// multiset, so the count of each type is conserved — vaults relocate, they are never
/// created or destroyed.
const BASE_TYPES: [RoomType; ROOM_COUNT] = [
    RoomType::KeystoneVault,
    RoomType::DeadEnd,
    RoomType::PowerCache,
    RoomType::Survey,
    RoomType::Control,
    RoomType::Reactor,
    RoomType::KeystoneVault,
    RoomType::Sensor,
    RoomType::Decoy,
];

/// Keystones the gate needs to unlock (all vaults — a tight objective so any stranded
/// keystone is immediately fatal, which makes the constraint's value crisp).
pub const REQUIRED_KEYSTONES: u32 = 2;
/// Power the gate needs to unlock (less than the available supply, so there is some slack).
pub const REQUIRED_POWER: u32 = 2;
pub const READER_BOT_SEEDS: u32 = 200;
pub const READER_BOT_TARGET_VISIT_MARGIN: f32 = 2.0;

/// The 4-neighbour rooms (up/down/left/right) of `room` on the facility grid.
pub fn neighbors(room: usize) -> Vec<usize> {
    let (col, row) = ((room % GRID_COLS) as isize, (room / GRID_COLS) as isize);
    let mut out = Vec::new();
    for (dc, dr) in [(-1isize, 0isize), (1, 0), (0, -1), (0, 1)] {
        let (c, r) = (col + dc, row + dr);
        if c >= 0 && c < GRID_COLS as isize && r >= 0 && r < GRID_ROWS as isize {
            out.push((r as usize) * GRID_COLS + c as usize);
        }
    }
    out
}

pub fn identity_for_room_type(room_type: RoomType) -> DoorIdentityRole {
    match room_type {
        RoomType::KeystoneVault => DoorIdentityRole::KeystoneVault,
        RoomType::PowerCache => DoorIdentityRole::PowerCache,
        RoomType::Reactor => DoorIdentityRole::Reactor,
        RoomType::Control => DoorIdentityRole::Control,
        RoomType::Survey => DoorIdentityRole::Survey,
        RoomType::Sensor => DoorIdentityRole::Sensor,
        RoomType::Decoy => DoorIdentityRole::Decoy,
        RoomType::DeadEnd => DoorIdentityRole::DeadEnd,
    }
}

pub fn door_read_for_room_type(room_type: RoomType, harvested: bool) -> DoorIdentityRole {
    if room_type == RoomType::Decoy && !harvested {
        DoorIdentityRole::FalseExit
    } else {
        identity_for_room_type(room_type)
    }
}

// Replaced duplicate SplitMix with shared observed_core::SplitMix

#[derive(Resource, Clone, Debug)]
pub struct DiscoveryWorld {
    /// Each room's current (real) type.
    pub types: Vec<RoomType>,
    /// Each room has been visited+harvested (its contents taken; it is now spent).
    pub harvested: Vec<bool>,
    /// The last type the team saw in each room (drives the schematic); `None` = unknown.
    /// For a Decoy this records the *displayed* lie when revealed remotely, and the truth
    /// once it is actually visited.
    pub known: Vec<Option<RoomType>>,
    pub known_source: Vec<Option<KnowledgeSource>>,
    /// The room the team currently occupies — observed, so it never shifts. `None` at
    /// the start (nothing observed yet).
    pub at: Option<usize>,
    pub keystones: u32,
    pub power: u32,
    /// A harvested Control room has stabilised the facility (shifting stops).
    pub stabilized: bool,
    pub escaped: bool,
    /// The solvability constraint: only shift types among **unharvested** rooms, so a
    /// keystone can never strand on a spent room. Toggle off to show why it exists.
    pub constraint_enabled: bool,
    pub base_seed: u64,
    pub shift_count: u32,
    pub visit_count: u32,
    pub sensor_map_updates: u32,
    pub decoy_lies_resolved: u32,
    /// Whether the objective is still completable (enough keystones/power collectable).
    pub solvable: bool,
    pub last_event: String,
}

impl DiscoveryWorld {
    pub fn authored() -> Self {
        let mut world = Self {
            types: BASE_TYPES.to_vec(),
            harvested: vec![false; ROOM_COUNT],
            known: vec![None; ROOM_COUNT],
            known_source: vec![None; ROOM_COUNT],
            at: None,
            keystones: 0,
            power: 0,
            stabilized: false,
            escaped: false,
            constraint_enabled: true,
            base_seed: 0x5EED_D15C_0FEE_1234,
            shift_count: 0,
            visit_count: 0,
            sensor_map_updates: 0,
            decoy_lies_resolved: 0,
            solvable: true,
            last_event: "Find the keystones; the exit is locked until you do.".to_string(),
        };
        world.recompute_solvability();
        world
    }

    pub fn reset(&mut self) {
        let constraint = self.constraint_enabled;
        *self = Self::authored();
        // Keep the demonstrator's constraint toggle across a reset.
        self.constraint_enabled = constraint;
    }

    /// The gate is open once enough keystones and power have been collected.
    pub fn gate_open(&self) -> bool {
        self.keystones >= REQUIRED_KEYSTONES && self.power >= REQUIRED_POWER
    }

    /// The type to *show* for a room when it is revealed remotely (Survey/Sensor): a Decoy
    /// that has not been visited lies and shows as a Keystone Vault; everything else shows
    /// its real type. A direct visit always records the truth (see [`Self::visit`]).
    pub fn displayed_type(&self, room: usize) -> RoomType {
        if !self.harvested[room] && self.types[room] == RoomType::Decoy {
            RoomType::KeystoneVault
        } else {
            self.types[room]
        }
    }

    /// The doorframe read available before committing to `room`. Decoys keep their true
    /// payoff hidden by presenting an exit-like signal until directly visited.
    pub fn door_read(&self, room: usize) -> Option<DoorIdentityRole> {
        (room < ROOM_COUNT).then(|| door_read_for_room_type(self.types[room], self.harvested[room]))
    }

    pub fn door_bleed_label(&self, room: usize) -> Option<&'static str> {
        self.door_read(room).map(DoorIdentityRole::ambience_label)
    }

    pub fn team_map_known_count(&self) -> usize {
        self.known.iter().filter(|entry| entry.is_some()).count()
    }

    /// Keystones still collectable: **real** Keystone Vaults sitting on unharvested rooms
    /// (Decoys are excluded — they never count, which is what keeps deception from
    /// affecting solvability).
    pub fn collectable_keystones(&self) -> u32 {
        (0..ROOM_COUNT)
            .filter(|&r| !self.harvested[r] && self.types[r] == RoomType::KeystoneVault)
            .count() as u32
    }

    /// Power still collectable: the summed yield of unharvested power rooms (caches +
    /// reactors).
    pub fn collectable_power(&self) -> u32 {
        (0..ROOM_COUNT)
            .filter(|&r| !self.harvested[r])
            .map(|r| self.types[r].power_yield())
            .sum()
    }

    /// Can the objective still be completed from here?
    pub fn recompute_solvability(&mut self) {
        self.solvable = self.escaped
            || (self.keystones + self.collectable_keystones() >= REQUIRED_KEYSTONES
                && self.power + self.collectable_power() >= REQUIRED_POWER);
    }

    /// Reveal `room`'s type into `known`, using the displayed (possibly lying) type — this
    /// is how Survey/Sensor learn rooms at a distance.
    fn reveal_from(&mut self, room: usize, source: KnowledgeSource) {
        self.known[room] = Some(self.displayed_type(room));
        if self.known_source[room] != Some(KnowledgeSource::Visit) {
            self.known_source[room] = Some(source);
        }
    }

    /// Visit a room: it becomes observed (frozen), and on the first visit it is harvested
    /// — its **true** type is revealed and its contents applied. Revisiting only re-reads
    /// the (now spent) type. Returns the true type found on a fresh harvest.
    pub fn visit(&mut self, room: usize) -> Option<RoomType> {
        if room >= ROOM_COUNT || self.escaped {
            return None;
        }
        self.at = Some(room);
        // A direct visit always learns the truth (overriding any earlier remote lie).
        self.known[room] = Some(self.types[room]);
        self.known_source[room] = Some(KnowledgeSource::Visit);
        if self.harvested[room] {
            self.last_event = format!("Room {room} is spent ({}).", self.types[room].label());
            return None;
        }
        self.harvested[room] = true;
        self.visit_count += 1;
        let found = self.types[room];
        match found {
            RoomType::KeystoneVault => {
                self.keystones += 1;
                self.last_event =
                    format!("Keystone Vault! {} / {REQUIRED_KEYSTONES}", self.keystones);
            }
            RoomType::PowerCache => {
                self.power += found.power_yield();
                self.last_event = format!("Power Cache tapped. power {}", self.power);
            }
            RoomType::Reactor => {
                self.power += found.power_yield();
                self.last_event = format!("Reactor online (+2). power {}", self.power);
            }
            RoomType::Control => {
                self.stabilized = true;
                self.last_event = "Control seized - the facility stops shifting.".to_string();
            }
            RoomType::Survey => {
                for r in 0..ROOM_COUNT {
                    self.reveal_from(r, KnowledgeSource::Survey);
                }
                self.last_event = "Survey ping - every room's current type revealed.".to_string();
            }
            RoomType::Sensor => {
                let mut updates = 0;
                for r in neighbors(room) {
                    if self.known_source[r] != Some(KnowledgeSource::Visit) {
                        updates += 1;
                    }
                    self.reveal_from(r, KnowledgeSource::Sensor);
                }
                self.sensor_map_updates += updates;
                self.last_event =
                    format!("Sensor sweep - {updates} adjacent rooms fed to the team map.");
            }
            RoomType::Decoy => {
                // The betrayal: it looked like a vault, it was a lie.
                self.decoy_lies_resolved += 1;
                self.last_event = format!("Room {room} was a DECOY - nothing here.");
            }
            RoomType::DeadEnd => {
                self.last_event = format!("Room {room}: dead end. Nothing here.");
            }
        }
        self.recompute_solvability();
        Some(found)
    }

    /// Try to leave through the gated exit; succeeds only once the gate is open.
    pub fn escape(&mut self) -> bool {
        if self.gate_open() {
            self.escaped = true;
            self.last_event = "Gate unlocked - escaped the facility!".to_string();
            true
        } else {
            self.last_event = format!(
                "Exit locked: need {REQUIRED_KEYSTONES} keystones (+{REQUIRED_POWER} power).",
            );
            false
        }
    }

    /// Decohere: permute the types among the **unobserved** rooms. With the constraint
    /// on, only *unharvested* rooms are in the pool (keystones can never strand on a spent
    /// room); with it off, harvested rooms are included and a keystone can be lost.
    pub fn shift(&mut self) {
        if self.stabilized || self.escaped {
            self.last_event = if self.stabilized {
                "Facility stabilised - types hold.".to_string()
            } else {
                self.last_event.clone()
            };
            return;
        }
        self.shift_count += 1;
        let pool: Vec<usize> = (0..ROOM_COUNT)
            .filter(|&r| Some(r) != self.at && (!self.constraint_enabled || !self.harvested[r]))
            .collect();

        // Fisher–Yates over the pooled rooms' types — a pure permutation, so the multiset
        // (the count of each type) is conserved.
        let mut values: Vec<RoomType> = pool.iter().map(|&r| self.types[r]).collect();
        let mut rng = SplitMix(
            self.base_seed ^ (self.shift_count as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15),
        );
        for i in (1..values.len()).rev() {
            values.swap(i, rng.below(i + 1));
        }
        for (slot, &room) in values.iter().zip(pool.iter()) {
            self.types[room] = *slot;
        }

        self.recompute_solvability();
        self.last_event = if self.solvable {
            format!("Rooms shifted ({} unseen). The layout changed.", pool.len())
        } else {
            "A keystone stranded on a spent room - objective lost!".to_string()
        };
    }

    /// Lowest-index room that is still worth visiting (unharvested), if any.
    pub fn next_unharvested(&self) -> Option<usize> {
        (0..ROOM_COUNT).find(|&r| !self.harvested[r])
    }
}

fn unharvested_rooms(world: &DiscoveryWorld) -> Vec<usize> {
    (0..ROOM_COUNT).filter(|&r| !world.harvested[r]).collect()
}

fn reader_score(world: &DiscoveryWorld, room: usize) -> i32 {
    let needs_key = world.keystones < REQUIRED_KEYSTONES;
    let needs_power = world.power < REQUIRED_POWER;
    match world.door_read(room).expect("room index is valid") {
        DoorIdentityRole::KeystoneVault if needs_key => 120,
        DoorIdentityRole::Reactor if needs_power => 105,
        DoorIdentityRole::PowerCache if needs_power => 95,
        DoorIdentityRole::Sensor if world.team_map_known_count() < ROOM_COUNT => 70,
        DoorIdentityRole::Survey if world.team_map_known_count() < ROOM_COUNT => 65,
        DoorIdentityRole::Control => 45,
        DoorIdentityRole::KeystoneVault => 35,
        DoorIdentityRole::Reactor | DoorIdentityRole::PowerCache => 25,
        DoorIdentityRole::FalseExit => {
            if world.gate_open() {
                80
            } else {
                -30
            }
        }
        DoorIdentityRole::Decoy => -35,
        DoorIdentityRole::DeadEnd => -45,
        DoorIdentityRole::Sensor | DoorIdentityRole::Survey => 10,
    }
}

fn choose_reader_room(world: &DiscoveryWorld) -> Option<usize> {
    unharvested_rooms(world)
        .into_iter()
        .max_by_key(|&room| (reader_score(world, room), std::cmp::Reverse(room)))
}

fn choose_random_room(world: &DiscoveryWorld, rng: &mut SplitMix) -> Option<usize> {
    let candidates = unharvested_rooms(world);
    candidates.get(rng.below(candidates.len())).copied()
}

pub fn run_bot(seed: u64, strategy: BotStrategy) -> BotRunOutcome {
    let mut world = DiscoveryWorld::authored();
    world.base_seed = seed ^ 0xD00A_D155_C0DE_0027;
    world.constraint_enabled = true;
    world.shift();
    let mut rng = SplitMix::new(seed ^ 0xA11C_E5CE_39B0_7A11);
    let mut wasted_visits = 0;

    for _ in 0..(ROOM_COUNT * 3) {
        if world.gate_open() {
            break;
        }
        let Some(room) = (match strategy {
            BotStrategy::Reader => choose_reader_room(&world),
            BotStrategy::Random => choose_random_room(&world, &mut rng),
        }) else {
            break;
        };
        let found = world.visit(room);
        if matches!(found, Some(RoomType::Decoy | RoomType::DeadEnd)) {
            wasted_visits += 1;
        }
        if !world.gate_open() {
            world.shift();
        }
    }
    let escaped = world.escape();

    BotRunOutcome {
        strategy,
        seed,
        escaped,
        visits: world.visit_count,
        shifts: world.shift_count,
        wasted_visits,
        sensor_map_updates: world.sensor_map_updates,
        decoy_lies_resolved: world.decoy_lies_resolved,
    }
}

pub fn compare_reader_and_random(seeds: u32) -> BotComparison {
    let mut reader_escapes = 0;
    let mut random_escapes = 0;
    let mut reader_visits = 0;
    let mut random_visits = 0;

    for seed in 0..seeds {
        let seed = seed as u64;
        let reader = run_bot(seed, BotStrategy::Reader);
        let random = run_bot(seed, BotStrategy::Random);
        reader_escapes += u32::from(reader.escaped);
        random_escapes += u32::from(random.escaped);
        reader_visits += reader.visits;
        random_visits += random.visits;
    }

    let seeds_f = seeds.max(1) as f32;
    let reader_mean_visits = reader_visits as f32 / seeds_f;
    let random_mean_visits = random_visits as f32 / seeds_f;
    BotComparison {
        seeds,
        reader_escapes,
        random_escapes,
        reader_mean_visits,
        random_mean_visits,
        visit_margin: random_mean_visits - reader_mean_visits,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn type_multiset(world: &DiscoveryWorld) -> Vec<char> {
        let mut g: Vec<char> = world.types.iter().map(|t| t.glyph()).collect();
        g.sort_unstable();
        g
    }

    #[test]
    fn the_gate_starts_locked_and_opens_only_with_enough_keystones_and_power() {
        let mut world = DiscoveryWorld::authored();
        assert!(!world.gate_open());
        assert!(!world.escape(), "cannot escape while locked");
        world.keystones = REQUIRED_KEYSTONES;
        assert!(!world.gate_open(), "keystones alone are not enough");
        world.power = REQUIRED_POWER;
        assert!(world.gate_open());
        assert!(world.escape());
        assert!(world.escaped);
    }

    #[test]
    fn a_rooms_type_is_unknown_until_it_is_visited() {
        let mut world = DiscoveryWorld::authored();
        assert!(world.known.iter().all(|k| k.is_none()), "nothing known yet");
        world.visit(0);
        assert_eq!(
            world.known[0],
            Some(world.types[0]),
            "visiting reveals the type"
        );
        assert!(world.harvested[0]);
    }

    #[test]
    fn the_observed_room_does_not_shift() {
        let mut world = DiscoveryWorld::authored();
        // Room 1 is an inert Dead-end — visiting it does not stabilise the facility, so the
        // *other* rooms keep shifting while this observed one stays frozen.
        world.visit(1);
        let here = world.types[1];
        for _ in 0..20 {
            world.visit(1); // stay put (re-observe), keeping room 1 frozen
            world.shift();
        }
        assert_eq!(world.types[1], here, "the observed room is frozen");
    }

    #[test]
    fn shifting_conserves_the_type_multiset() {
        let mut world = DiscoveryWorld::authored();
        let before = type_multiset(&world);
        for _ in 0..50 {
            world.shift();
            assert_eq!(
                type_multiset(&world),
                before,
                "types are permuted, not changed"
            );
        }
    }

    #[test]
    fn shifting_is_deterministic() {
        let mut a = DiscoveryWorld::authored();
        let mut b = DiscoveryWorld::authored();
        a.visit(0);
        b.visit(0);
        for _ in 0..10 {
            a.shift();
            b.shift();
        }
        assert_eq!(a.types, b.types);
    }

    /// With the constraint on, a full sweep always collects every keystone and opens the
    /// gate — across seeds and with the facility shifting between every visit.
    #[test]
    fn with_the_constraint_the_objective_is_always_solvable() {
        for seed in 0..60u64 {
            let mut world = DiscoveryWorld::authored();
            world.base_seed = seed;
            world.constraint_enabled = true;
            for _ in 0..(ROOM_COUNT * 2) {
                if world.gate_open() {
                    break;
                }
                if let Some(r) = world.next_unharvested() {
                    world.visit(r);
                }
                assert!(
                    world.solvable,
                    "the objective must never become impossible (seed {seed})"
                );
                world.shift();
            }
            assert!(
                world.escape(),
                "a full sweep unlocks the gate (seed {seed})"
            );
        }
    }

    /// Without the constraint, the same shifting can strand a keystone on a spent room and
    /// make the objective impossible — which is exactly why the constraint exists.
    #[test]
    fn without_the_constraint_a_keystone_can_strand_and_doom_the_run() {
        let mut ever_unsolvable = false;
        for seed in 0..120u64 {
            let mut world = DiscoveryWorld::authored();
            world.base_seed = seed;
            world.constraint_enabled = false;
            for _ in 0..(ROOM_COUNT * 2) {
                if let Some(r) = world.next_unharvested() {
                    world.visit(r);
                }
                world.shift();
                if !world.solvable {
                    ever_unsolvable = true;
                    break;
                }
            }
            if ever_unsolvable {
                break;
            }
        }
        assert!(
            ever_unsolvable,
            "without the constraint, some run should strand a keystone (that's why it exists)"
        );
    }

    #[test]
    fn control_stabilises_and_survey_reveals() {
        let mut world = DiscoveryWorld::authored();
        // Control sits at room 4 initially; with nothing observed yet it is harvestable.
        world.visit(4);
        assert!(world.stabilized);
        let snapshot = world.types.clone();
        world.shift(); // stabilised → no change
        assert_eq!(world.types, snapshot, "stabilised facility holds");

        // Survey reveals the current types of every room at once.
        let mut w2 = DiscoveryWorld::authored();
        w2.visit(3); // Survey at room 3
        assert!(
            w2.known.iter().all(|k| k.is_some()),
            "survey reveals all types"
        );
    }

    #[test]
    fn reactor_yields_two_power_and_collectable_power_sums_yields() {
        let mut world = DiscoveryWorld::authored();
        // Cache (1) + Reactor (2) = 3 power available before anything is harvested.
        assert_eq!(world.collectable_power(), 3);
        // Reactor sits at room 5 initially.
        assert_eq!(world.types[5], RoomType::Reactor);
        world.visit(5);
        assert_eq!(world.power, 2, "a reactor yields two power");
        assert_eq!(
            world.collectable_power(),
            1,
            "only the power cache remains collectable"
        );
    }

    #[test]
    fn a_sensor_reveals_only_its_neighbours() {
        let mut world = DiscoveryWorld::authored();
        // Sensor sits at room 7 (bottom-middle); neighbours are 4 (up), 6 (left), 8 (right).
        assert_eq!(world.types[7], RoomType::Sensor);
        let mut nb = neighbors(7);
        nb.sort_unstable();
        assert_eq!(nb, vec![4, 6, 8]);
        world.visit(7);
        for r in [4, 6, 8] {
            assert!(world.known[r].is_some(), "neighbour {r} revealed");
            assert_eq!(
                world.known_source[r],
                Some(KnowledgeSource::Sensor),
                "neighbour {r} is marked as sensor-fed map knowledge",
            );
        }
        for r in [0, 1, 2, 3, 5] {
            assert!(world.known[r].is_none(), "non-neighbour {r} stays unknown");
        }
        assert_eq!(world.sensor_map_updates, 3);
    }

    #[test]
    fn a_decoy_lies_when_revealed_but_yields_nothing_and_is_never_a_real_keystone() {
        let mut world = DiscoveryWorld::authored();
        // Decoy sits at room 8 initially; only the two real vaults count as collectable.
        assert_eq!(world.types[8], RoomType::Decoy);
        assert_eq!(
            world.collectable_keystones(),
            2,
            "the decoy is not a keystone"
        );
        assert_eq!(
            world.door_read(8),
            Some(DoorIdentityRole::FalseExit),
            "an unvisited decoy advertises an exit-like door read",
        );
        assert_eq!(world.door_bleed_label(8), Some("exit choir"));

        // Revealed remotely (here via Survey at room 3) the decoy *displays* as a vault.
        world.visit(3);
        assert_eq!(
            world.known[8],
            Some(RoomType::KeystoneVault),
            "a remotely-revealed decoy lies"
        );

        // Visiting it learns the truth and yields no keystone.
        let before = world.keystones;
        let found = world.visit(8);
        assert_eq!(found, Some(RoomType::Decoy));
        assert_eq!(world.keystones, before, "a decoy yields no keystone");
        assert_eq!(world.decoy_lies_resolved, 1);
        assert_eq!(
            world.known[8],
            Some(RoomType::Decoy),
            "the visit reveals truth"
        );
        assert_eq!(
            world.door_read(8),
            Some(DoorIdentityRole::Decoy),
            "once resolved, the door read no longer masquerades as an exit",
        );
    }

    #[test]
    fn door_identity_reads_are_style_owned_and_type_true() {
        let world = DiscoveryWorld::authored();
        for room in 0..ROOM_COUNT {
            let read = world.door_read(room).expect("room has a read");
            if world.types[room] == RoomType::Decoy {
                assert_eq!(read, DoorIdentityRole::FalseExit);
            } else {
                assert_eq!(read, identity_for_room_type(world.types[room]));
            }
        }
    }

    #[test]
    fn reader_bot_beats_random_door_bot_by_the_phase_margin() {
        let comparison = compare_reader_and_random(READER_BOT_SEEDS);
        assert_eq!(comparison.reader_escapes, READER_BOT_SEEDS);
        assert_eq!(comparison.random_escapes, READER_BOT_SEEDS);
        assert!(
            comparison.visit_margin >= READER_BOT_TARGET_VISIT_MARGIN,
            "reader mean {:.2}, random mean {:.2}, margin {:.2} below target {:.2}",
            comparison.reader_mean_visits,
            comparison.random_mean_visits,
            comparison.visit_margin,
            READER_BOT_TARGET_VISIT_MARGIN,
        );
    }

    #[test]
    fn reset_restores_the_authored_facility() {
        let mut world = DiscoveryWorld::authored();
        for _ in 0..5 {
            if let Some(r) = world.next_unharvested() {
                world.visit(r);
            }
            world.shift();
        }
        world.reset();
        assert_eq!(world.shift_count, 0);
        assert_eq!(world.visit_count, 0);
        assert_eq!(world.keystones, 0);
        assert_eq!(world.sensor_map_updates, 0);
        assert_eq!(world.decoy_lies_resolved, 0);
        assert!(!world.escaped);
        assert!(world.solvable);
        assert!(world.known.iter().all(|k| k.is_none()));
        assert!(world.known_source.iter().all(|source| source.is_none()));
    }
}
