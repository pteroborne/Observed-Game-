//! A scripted driver that plays the lab, for recording evidence.
//!
//! The first recording tape was a list of timed intents — "look down at tick
//! 91" — which is fine for a seven second shot of one shove and useless for
//! anything that has to *navigate*. Walking to the generator takes a number of
//! ticks that depends on where you started, whether you clipped a wall, and how
//! long the turn took, so blind timing drifts and the rest of the tape lands on
//! the wrong beats.
//!
//! This driver is closed-loop instead: each step reads authoritative state and
//! produces a [`PlayerIntent`] for this tick, exactly as a bot would. That is
//! still fully deterministic — same board, same script, same frames — and it is
//! the same command path a player's hands use, so the recording stays a real run
//! of the rules rather than an animation of them.
//!
//! Every step carries a caption that is drawn on screen while it runs, so the
//! video states the claim it is demonstrating instead of asking a viewer to take
//! the README's word for it.

use bevy::prelude::*;
use observed_hex::coords::HexCoord;
use player_input::PlayerIntent;

use crate::embodied::{Embodiment, ToolRequest, plate_center};
use crate::model::KineticWorld;

/// Radians per tick the driver will turn. Comfortably readable on camera.
const TURN_RATE: f32 = 0.045;
/// How close to a plate's centre counts as arrived.
const ARRIVE_RADIUS: f32 = 2.5;
/// Ticks any navigation step may take before the driver gives up and moves on,
/// so one blocked path cannot strand the whole recording.
const NAV_TIMEOUT: u32 = 1_200;

/// One beat of the script.
#[derive(Clone, Copy, Debug)]
pub enum Beat {
    /// Hold still for a number of ticks.
    Hold(u32),
    /// Turn on the spot to look at a cell.
    LookAt(HexCoord),
    /// Walk to a cell, steering as it goes.
    WalkTo {
        target: HexCoord,
        sprint: bool,
    },
    /// Walk toward a cell without requiring arrival — used to leave the floor,
    /// where "arriving" means falling.
    WalkOff {
        target: HexCoord,
        ticks: u32,
    },
    Push,
    Pull,
    Generator,
    Jump,
    /// Move a Guardian. Staging, and captioned as such on screen.
    ///
    /// `stagger` holds it still for that many ticks. A staggered Guardian does
    /// not pursue — the model already works that way after a shove — which is
    /// the only honest lever the demo has for keeping an actor out of a scene it
    /// is not part of. Parking one "far away" does not work: the board is nine
    /// plates wide and a minor crosses one every 150 ticks, so every corner is
    /// within reach of a seventy-second recording.
    PlaceMinor {
        index: usize,
        cell: HexCoord,
        stagger: u32,
    },
    /// Park the major Guardian. It pursues throughout, and a systems tour takes
    /// long enough for it to cross the whole board and end the run mid-sentence.
    PlaceMajor {
        cell: HexCoord,
    },
    Reset,
}

/// A captioned beat.
pub struct Scene {
    pub caption: &'static str,
    pub beat: Beat,
}

const fn scene(caption: &'static str, beat: Beat) -> Scene {
    Scene { caption, beat }
}

const fn cell(q: u16, r: u16) -> HexCoord {
    HexCoord { q, r, level: 0 }
}

/// The full evidence script.
///
/// Ordered so each claim is visible before it is relied on: aiming feedback
/// first, then the two shove outcomes, then the systems that gate the tool, then
/// the two ways a run ends.
pub fn script() -> Vec<Scene> {
    vec![
        // The lethal push comes first on purpose. A Guardian in the lane is
        // *walking toward you the whole time* — three plates away is 450 ticks
        // of grace, and an earlier draft spent all of it on preamble and got
        // jailed before it ever pulled the trigger. Demonstrate the shot while
        // the target is still there, then take the slow beats afterwards.
        scene("Kinetic tool lab - first person", Beat::Hold(45)),
        scene(
            "Target in the lane: crosshair green - this push kills",
            Beat::Hold(85),
        ),
        scene("PUSH", Beat::Push),
        scene(
            "Momentum outlived the impulse: past the ledges, into void",
            Beat::Hold(220),
        ),
        scene(
            "Turn away: the lane is still drawn, with no target in it",
            Beat::LookAt(cell(3, 1)),
        ),
        scene("Crosshair dim - nothing to push", Beat::Hold(90)),
        scene("A refused push says so", Beat::Push),
        scene("...'Nothing in the lane.'", Beat::Hold(90)),
        // First re-park. Six plates buys 1440 ticks and act one runs to roughly
        // 1700, so without this the major walks into the back of the recharge
        // demonstration and ends the run fifteen ticks before its own parking
        // beat would have fired.
        scene(
            "Staged: the major parked out of the scene",
            Beat::PlaceMajor { cell: cell(7, 5) },
        ),
        scene(
            "Staged: a second Guardian on solid ground",
            Beat::PlaceMinor {
                index: 1,
                cell: cell(5, 4),
                // Held while the Observer walks over. Left free it closes one
                // plate during the approach, and then the pull below has no
                // room: pulling a target onto the cell you are standing on is
                // Blocked, correctly, and the demo shows nothing.
                stagger: 500,
            },
        ),
        scene(
            "Walk to line it up",
            Beat::WalkTo {
                target: cell(3, 4),
                sprint: false,
            },
        ),
        scene(
            "White crosshair: a target, but this push only staggers",
            Beat::LookAt(cell(5, 4)),
        ),
        // Pull first. A push lands it three plates out, which is past
        // `TOOL_RANGE`, so pulling afterwards finds nothing in the lane.
        scene("PULL drags it one plate closer", Beat::Pull),
        scene("", Beat::Hold(110)),
        scene("PUSH onto solid floor", Beat::Push),
        scene(
            "It survives, staggered - the floor is not lethal",
            Beat::Hold(170),
        ),
        scene("E away from the generator is refused", Beat::Generator),
        scene("...'Stand on the generator to operate it.'", Beat::Hold(90)),
        scene(
            "Walk to the recharge station",
            Beat::WalkTo {
                target: cell(2, 4),
                sprint: false,
            },
        ),
        scene(
            "Standing on a live station restores charge",
            Beat::Hold(190),
        ),
        // Clear the board before the systems tour. Guardians pursue the whole
        // time — a minor crosses a plate every 150 ticks and the major every
        // 240 — so the walking beats below are long enough for either to arrive
        // and end the run mid-sentence. Earlier cuts of this script were jailed
        // twice for exactly that reason.
        scene(
            "Staged: lined up on the west rim",
            Beat::PlaceMinor {
                index: 1,
                cell: cell(1, 4),
                stagger: 200,
            },
        ),
        scene(
            "Same tool, a lethal lane this time",
            Beat::LookAt(cell(1, 4)),
        ),
        scene("PUSH", Beat::Push),
        scene("Board clear", Beat::Hold(170)),
        scene(
            "Staged: the major parked, so the tour is not interrupted",
            Beat::PlaceMajor { cell: cell(7, 1) },
        ),
        scene(
            "Walk to the generator",
            Beat::WalkTo {
                target: cell(1, 1),
                sprint: true,
            },
        ),
        scene("Cut the power", Beat::Generator),
        scene(
            "Stations dead, sight is your own plate, the major wakes",
            Beat::Hold(170),
        ),
        scene("Power back on", Beat::Generator),
        scene("Sight returns", Beat::Hold(90)),
        scene("SPACE jumps", Beat::Jump),
        scene("", Beat::Hold(50)),
        // Re-park before the last stretch: the major has been walking since the
        // previous parking and the remaining beats are another ~800 ticks.
        scene(
            "Staged: the major parked again",
            Beat::PlaceMajor { cell: cell(1, 5) },
        ),
        scene(
            "The edge cuts both ways",
            Beat::WalkTo {
                target: cell(6, 3),
                sprint: true,
            },
        ),
        scene(
            "Walk off the rim",
            Beat::WalkOff {
                target: cell(9, 3),
                // From (6,3) the last solid ground ends 21 metres out, which is
                // 274 ticks at walking pace — a 260-tick beat stopped one stride
                // short of the edge, so nothing fell and the finale that depends
                // on respawning at the spawn plate never fired either.
                ticks: 500,
            },
        ),
        scene(
            "The void claims you, and returns you to spawn",
            Beat::Hold(120),
        ),
        scene(
            "Staged: a Guardian one plate away",
            Beat::PlaceMinor {
                index: 1,
                cell: cell(4, 3),
                stagger: 0,
            },
        ),
        scene("A Guardian reaching you ends the run", Beat::Hold(200)),
        scene("JAILED - no longer a silent freeze", Beat::Hold(120)),
        scene("R resets the board", Beat::Reset),
        scene("Everything restored", Beat::Hold(120)),
    ]
}

/// Place the actors for the recording.
///
/// Shared by the renderer and the headless runner so the video and the test can
/// never diverge on where things started.
pub fn stage(world: &mut KineticWorld, body: &mut Embodiment) {
    let stand = plate_center(cell(3, 3));
    body.body.position = Vec3::new(
        stand.x,
        crate::embodied::FLOOR_TOP + body.config.half_height,
        stand.z,
    );
    body.body.velocity = Vec3::ZERO;
    body.body.spawn = body.body.position;
    // Already looking down the ledge run, so the recording opens on the shot
    // that matters instead of on a second of turning.
    body.body.yaw = std::f32::consts::FRAC_PI_2;
    body.body.spawn_yaw = body.body.yaw;
    body.facing = observed_hex::faces::HexFace::East;

    world.observers[0].cell = cell(3, 3);
    world.observers[0].facing = observed_hex::faces::HexFace::East;
    // Three plates east: inside `TOOL_RANGE`, with the ledge run and the void
    // rim behind it, and 450 ticks of walking before it could reach the spawn.
    world.minors[0].cell = cell(6, 3);
    // Held still in the far corner until the script wants it. Distance alone is
    // not enough: two plates is 300 ticks and it walked into the back of the
    // opening shot; even six plates is only 900.
    world.minors[1].cell = cell(1, 5);
    world.minors[1].stagger = HOLD;
    // The major has no stagger, so distance is the only lever and it gets
    // re-parked between acts. From spawn the corners are 4, 4, 2 and 6 plates
    // away; at 240 ticks each only the six-plate corner outlasts the first act,
    // so the held minor takes one of the near corners and the major takes this.
    world.major.cell = cell(7, 5);
}

/// Long enough to hold a Guardian still for a whole recording.
pub const HOLD: u32 = 100_000;

/// One logged moment of a headless run.
#[derive(Clone, Debug)]
pub struct Logged {
    pub tick: u32,
    pub caption: String,
    pub event: String,
}

/// Play the whole script with no window, no renderer, and no frame timing.
///
/// This is the answer to "can the demo be verified without watching it": the
/// director, the model and the body are all pure enough to run flat out, so a
/// seventy-second recording can be checked in milliseconds and a broken beat
/// names itself instead of having to be spotted in a frame.
pub fn run_headless(max_ticks: u32) -> Vec<Logged> {
    let mut world = KineticWorld::authored();
    let mut body = Embodiment::new(&world);
    stage(&mut world, &mut body);

    let mut director = Director::default();
    let mut log = Vec::new();

    for tick in 0..max_ticks {
        if director.finished {
            break;
        }
        let caption = director.caption.clone();
        let decision = director.tick(&world, &body);

        if let Some((index, cell, stagger)) = decision.stage
            && let Some(minor) = world.minors.get_mut(index)
        {
            minor.cell = cell;
            minor.alive = true;
            minor.stagger = stagger;
            minor.step_progress = 0;
        }
        if let Some(cell) = decision.stage_major {
            world.major.cell = cell;
            world.major.step_progress = 0;
        }
        if decision.reset {
            world = KineticWorld::authored();
            body = Embodiment::new(&world);
            log.push(Logged {
                tick,
                caption: caption.clone(),
                event: "Reset".to_string(),
            });
            continue;
        }

        body.step(&mut world, decision.intent, decision.request);

        if body.fell_into_void {
            log.push(Logged {
                tick,
                caption: caption.clone(),
                event: "FellIntoVoid".to_string(),
            });
        }
        for event in &world.events {
            log.push(Logged {
                tick,
                caption: caption.clone(),
                event: format!("{event:?}"),
            });
        }
    }

    log
}

/// Progress through the script.
#[derive(Resource)]
pub struct Director {
    scenes: Vec<Scene>,
    pub index: usize,
    pub elapsed: u32,
    pub caption: String,
    pub finished: bool,
}

impl Default for Director {
    fn default() -> Self {
        let scenes = script();
        let caption = scenes
            .first()
            .map_or(String::new(), |scene| scene.caption.to_string());
        Self {
            scenes,
            index: 0,
            elapsed: 0,
            caption,
            finished: false,
        }
    }
}

/// What the driver decided this tick.
pub struct DirectorTick {
    pub intent: PlayerIntent,
    pub request: ToolRequest,
    pub reset: bool,
    pub stage: Option<(usize, HexCoord, u32)>,
    pub stage_major: Option<HexCoord>,
}

impl Director {
    fn advance(&mut self) {
        self.index += 1;
        self.elapsed = 0;
        match self.scenes.get(self.index) {
            Some(scene) => {
                if !scene.caption.is_empty() {
                    self.caption = scene.caption.to_string();
                }
            }
            None => self.finished = true,
        }
    }

    /// Produce this tick's input from current state.
    pub fn tick(&mut self, world: &KineticWorld, body: &Embodiment) -> DirectorTick {
        let mut out = DirectorTick {
            intent: PlayerIntent::default(),
            request: ToolRequest::None,
            reset: false,
            stage: None,
            stage_major: None,
        };
        let Some(scene) = self.scenes.get(self.index) else {
            self.finished = true;
            return out;
        };
        let beat = scene.beat;
        self.elapsed += 1;

        match beat {
            Beat::Hold(ticks) => {
                if self.elapsed >= ticks {
                    self.advance();
                }
            }
            Beat::LookAt(target) => {
                let done = self.steer(&mut out, world, body, target, false);
                // Turning is finished when the yaw error is inside one tick of
                // turn, or the step has taken implausibly long.
                if done || self.elapsed > 240 {
                    self.advance();
                }
            }
            Beat::WalkTo { target, sprint } => {
                let aligned = self.steer(&mut out, world, body, target, true);
                out.intent.sprint_held = sprint;
                if aligned {
                    out.intent.movement = Vec2::new(0.0, 1.0);
                }
                let here = plate_center(target);
                let delta = Vec2::new(here.x - body.body.position.x, here.z - body.body.position.z);
                if delta.length() < ARRIVE_RADIUS || self.elapsed > NAV_TIMEOUT {
                    self.advance();
                }
            }
            Beat::WalkOff { target, ticks } => {
                let aligned = self.steer(&mut out, world, body, target, true);
                if aligned {
                    out.intent.movement = Vec2::new(0.0, 1.0);
                }
                if self.elapsed >= ticks {
                    self.advance();
                }
            }
            Beat::Push => {
                out.request = ToolRequest::Push;
                self.advance();
            }
            Beat::Pull => {
                out.request = ToolRequest::Pull;
                self.advance();
            }
            Beat::Generator => {
                out.request = ToolRequest::ToggleGenerator;
                self.advance();
            }
            Beat::Jump => {
                out.intent.jump_pressed = true;
                self.advance();
            }
            Beat::PlaceMinor {
                index,
                cell,
                stagger,
            } => {
                out.stage = Some((index, cell, stagger));
                self.advance();
            }
            Beat::PlaceMajor { cell } => {
                out.stage_major = Some(cell);
                self.advance();
            }
            Beat::Reset => {
                out.reset = true;
                self.advance();
            }
        }

        out
    }

    /// Turn toward `target`, returning whether the body is now looking at it.
    ///
    /// `walking` widens the tolerance: steering while moving does not need the
    /// precision that lining up a shot does, and a tight tolerance makes the
    /// camera visibly hunt left and right as it walks.
    fn steer(
        &self,
        out: &mut DirectorTick,
        _world: &KineticWorld,
        body: &Embodiment,
        target: HexCoord,
        walking: bool,
    ) -> bool {
        let here = plate_center(target);
        let delta = Vec2::new(here.x - body.body.position.x, here.z - body.body.position.z);
        if delta.length_squared() < 1e-3 {
            return true;
        }
        // `FpsBody::forward` is (sin yaw, -cos yaw), so this inverts it.
        let desired = delta.x.atan2(-delta.y);
        let error = wrap_angle(desired - body.body.yaw);
        let tolerance = if walking { 0.08 } else { TURN_RATE };
        if error.abs() <= tolerance {
            return true;
        }
        // The scripted path feeds `look` straight to the controller, which
        // multiplies by `look_step`, so divide it back out to get radians.
        let step = error.clamp(-TURN_RATE, TURN_RATE);
        out.intent.look = Vec2::new(step / body.config.look_step, 0.0);
        false
    }
}

fn wrap_angle(angle: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    let wrapped = angle.rem_euclid(tau);
    if wrapped > std::f32::consts::PI {
        wrapped - tau
    } else {
        wrapped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_angle_folds_into_half_turns() {
        let pi = std::f32::consts::PI;
        assert!((wrap_angle(0.0)).abs() < 1e-6);
        assert!((wrap_angle(pi * 2.0)).abs() < 1e-5);
        assert!((wrap_angle(pi * 1.5) + pi * 0.5).abs() < 1e-5);
        assert!((wrap_angle(-pi * 1.5) - pi * 0.5).abs() < 1e-5);
        for steps in -20..20 {
            let angle = steps as f32 * 0.7;
            assert!(wrap_angle(angle).abs() <= pi + 1e-5);
        }
    }

    /// Print the whole run. Not an assertion — a debugging instrument, kept
    /// because reading a beat-by-beat log is how every scripting bug in this
    /// demo was actually found.
    #[test]
    #[ignore = "diagnostic: cargo test -p kinetic_lab -- --ignored --nocapture script_log"]
    fn script_log() {
        for entry in run_headless(30_000) {
            println!("{:>6}  {:<58}  {}", entry.tick, entry.caption, entry.event);
        }
    }

    /// The recording demonstrates what it claims to demonstrate.
    ///
    /// This is the headless counterpart of the video: it runs the identical
    /// script through the identical command path with no window and no frame
    /// timing, and asserts that each thing the captions promise actually
    /// happened, in order. Three separate cuts of this script were silently
    /// broken — jailed at tick 299, then 982, then 1685, and one that stopped a
    /// stride short of the edge so the last third never ran — and each was found
    /// here in milliseconds rather than by watching seventy seconds of frames.
    #[test]
    fn the_scripted_demo_demonstrates_every_claim_it_makes() {
        let log = run_headless(30_000);
        let events: Vec<&str> = log.iter().map(|entry| entry.event.as_str()).collect();

        let find = |from: usize, needle: &str| -> Option<usize> {
            events
                .iter()
                .enumerate()
                .skip(from)
                .find(|(_, event)| event.contains(needle))
                .map(|(index, _)| index)
        };

        // In order: the lethal push, a refusal, a pull, a survivable push, the
        // generator refusal, recharge, power down and back up, a fall into void,
        // a capture, and the reset.
        let mut at = 0;
        for needle in [
            "fate: Void",
            "GuardianDestroyed",
            "NoTargetInLane",
            "face: West, cells_travelled: 1, fate: Rest",
            "fate: Rest",
            "NotOnGenerator",
            "ChargeRestored",
            "powered: false",
            "powered: true",
            "FellIntoVoid",
            "ObserverCaptured",
            "Reset",
        ] {
            at = find(at, needle)
                .unwrap_or_else(|| panic!("the script never demonstrates {needle}\n{events:#?}"))
                + 1;
        }

        // The Observer must survive the whole demonstration and only be jailed
        // by the finale that is about being jailed. An early capture freezes the
        // board and silently voids every beat after it.
        let first_capture = find(0, "ObserverCaptured").expect("the finale jails the Observer");
        let fell = find(0, "FellIntoVoid").expect("the Observer falls into void");
        assert!(
            fell < first_capture,
            "captured before the script finished: the run was jailed at log entry \
             {first_capture}, which freezes the board and voids everything after it"
        );

        assert!(
            log.last().is_some_and(|entry| entry.event == "Reset"),
            "the script must end by resetting, so the recording closes on a clean board"
        );
    }

    /// The script must terminate on its own: every navigation beat has a
    /// timeout, so a blocked path cannot strand a recording forever.
    #[test]
    fn every_navigation_beat_is_bounded() {
        for scene in script() {
            match scene.beat {
                Beat::WalkTo { .. } | Beat::LookAt(_) => {}
                Beat::WalkOff { ticks, .. } => assert!(ticks > 0),
                Beat::Hold(ticks) => assert!(ticks > 0),
                _ => {}
            }
        }
        assert!(!script().is_empty());
    }
}
