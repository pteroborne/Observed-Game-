//! The overview's things: drawn on the storey it shows, within its reach, and
//! nowhere else.
//!
//! The cutaway filters hulls and lights by storey, and that was all it filtered.
//! Everything else the facility holds - generators and recharge stations, the
//! objective mechanisms, lanterns, doors, sensors, the Guardians major and
//! minor, the exit beacon, every room's threshold gate - is drawn by its own
//! module wherever it stands, resident or not. From inside a corridor that is
//! right. From the overview it drew every floor's furniture at once, hanging in
//! the sky around a plan that shows one: arches that were threshold gates three storeys up, red streaks
//! that were minors on the floor below.
//!
//! # Hidden, and given back exactly
//!
//! Most of these modules set their own visibility - a dead minor, an unpowered
//! station's column, a lantern in a cache. So the overview never decides that a
//! thing is *shown*: it only hides what is off its storey, remembers what the
//! thing was, and gives that back when the storey comes round again or the
//! overview goes down. It runs after `Update`, so a module that rewrites its
//! visibility every frame is overruled for that frame rather than fought.
//!
//! The followed body's own marks belong to `cutaway_marks`, which already draws
//! them for exactly this view; they are not here.

use bevy::camera::visibility::VisibilitySystems;
use bevy::prelude::*;
use bevy::transform::TransformSystems;

use super::SpectatorOverview;
use crate::hex_wfc::sim::HexWfcRuntime;
use crate::hex_wfc::{doors, entities, guardian, kinetic, lantern, power, prison_gate, sensors};

/// Everything the overview holds to its storey.
type Prop = Or<(
    With<power::PowerFixture>,
    With<lantern::LanternVisual>,
    With<guardian::HexGuardianVisual>,
    With<guardian::released::ReleasedVisual>,
    With<sensors::SensorVisual>,
    With<prison_gate::LobbyGate>,
    With<entities::ObjectiveVisual>,
    With<entities::ActorVisual>,
    With<entities::ExitBeacon>,
    With<kinetic::HeldTool>,
    With<doors::DoorVisual>,
    With<super::super::thresholds::ThresholdFrame>,
)>;

/// What a prop's visibility was when the overview hid it.
#[derive(Component)]
pub(in crate::hex_wfc) struct HeldByOverview(Visibility);

/// After transforms are propagated, so a prop is judged where it stands this
/// frame; before visibility is, so the hiding takes effect this frame too.
pub(in crate::hex_wfc) fn schedule(app: &mut App) {
    app.add_systems(
        PostUpdate,
        sync.after(TransformSystems::Propagate)
            .before(VisibilitySystems::VisibilityPropagate)
            .run_if(in_state(crate::GameState::HexWfc)),
    );
}

fn sync(
    mut commands: Commands,
    runtime: Res<HexWfcRuntime>,
    overview: Res<SpectatorOverview>,
    mut props: Query<
        (
            Entity,
            &GlobalTransform,
            &mut Visibility,
            Option<&HeldByOverview>,
        ),
        Prop,
    >,
) {
    let window = overview.active.then(|| {
        let (low, high) = super::super::camera::storey(runtime.local().cell.level);
        let centre = super::super::camera::tile_centre(runtime.local().cell).xz();
        let reach = super::super::camera::detail_reach(overview.tile_radius);
        move |at: Vec3| at.y >= low && at.y < high && at.xz().distance(centre) <= reach
    });
    for (entity, transform, mut visibility, held) in &mut props {
        let shown = window
            .as_ref()
            .is_none_or(|holds| holds(transform.translation()));
        match judge(shown, *visibility, held.map(|held| held.0)) {
            Hold::Leave => {}
            Hold::Hide(was) => {
                commands.entity(entity).insert(HeldByOverview(was));
                *visibility = Visibility::Hidden;
            }
            Hold::GiveBack(was) => {
                *visibility = was;
                commands.entity(entity).remove::<HeldByOverview>();
            }
        }
    }
}

/// What the overview does to one prop this frame.
#[derive(Debug, PartialEq)]
enum Hold {
    Leave,
    /// Hide it, remembering what it was.
    Hide(Visibility),
    /// Stop holding it, and set it to this.
    GiveBack(Visibility),
}

/// Whether a prop the overview should `show` is hidden or given back, from its
/// visibility `now` and what the overview remembered of it when it was `held`.
fn judge(show: bool, now: Visibility, held: Option<Visibility>) -> Hold {
    match (show, held) {
        (false, None) => Hold::Hide(now),
        // The prop changed its own mind while hidden: that is what it goes back to.
        (false, Some(_)) if now != Visibility::Hidden => Hold::Hide(now),
        (true, Some(was)) => Hold::GiveBack(if now == Visibility::Hidden { was } else { now }),
        _ => Hold::Leave,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_prop_off_the_storey_is_hidden_and_given_back_as_it_was() {
        let held = match judge(false, Visibility::Visible, None) {
            Hold::Hide(was) => was,
            other => panic!("an off-storey prop must be hidden, not {other:?}"),
        };
        assert_eq!(judge(false, Visibility::Hidden, Some(held)), Hold::Leave);
        assert_eq!(
            judge(true, Visibility::Hidden, Some(held)),
            Hold::GiveBack(Visibility::Visible)
        );
    }

    #[test]
    fn a_prop_that_hid_itself_stays_hidden_when_its_storey_comes_round() {
        // A dead minor, say: hidden by its own module before the overview held it.
        assert_eq!(
            judge(false, Visibility::Hidden, None),
            Hold::Hide(Visibility::Hidden)
        );
        assert_eq!(
            judge(true, Visibility::Hidden, Some(Visibility::Hidden)),
            Hold::GiveBack(Visibility::Hidden)
        );
    }

    #[test]
    fn what_a_prop_decides_while_held_is_what_it_gets_back() {
        // Held while shown, then its module hides it (overwriting the overview's
        // Hidden with its own): that is remembered instead.
        assert_eq!(
            judge(false, Visibility::Inherited, Some(Visibility::Visible)),
            Hold::Hide(Visibility::Inherited)
        );
        // Its module sets it shown in the frame the storey comes back: kept.
        assert_eq!(
            judge(true, Visibility::Visible, Some(Visibility::Hidden)),
            Hold::GiveBack(Visibility::Visible)
        );
    }

    #[test]
    fn nothing_is_touched_while_the_overview_is_down() {
        for now in [
            Visibility::Visible,
            Visibility::Hidden,
            Visibility::Inherited,
        ] {
            assert_eq!(judge(true, now, None), Hold::Leave);
        }
    }
}
