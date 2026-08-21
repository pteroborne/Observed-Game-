//! Tap targets for the actions a phone otherwise cannot reach.
//!
//! Every tab except Tuning renders as one formatted string in a single `Text`
//! node, driven by global keyboard shortcuts. That is fine at a desk and leaves
//! nothing to touch on a handset: seeds, floors and the rest are reachable only
//! by keys that do not exist there.
//!
//! Converting those panels into real widget rows - what `field_widgets` already
//! does for Tuning - is the proper fix and a large one. This is the smaller
//! thing that unblocks the work in the meantime: the handful of controls needed
//! to actually *browse* generated facilities, which is what the studio is for.
//! Seeds first, because comparing layouts across seeds is the whole loop.

use bevy::prelude::*;
use observed_style::{SchematicRole, schematic};

use crate::chrome_layout::CompactOnly;
use crate::typography;

/// Height of a tap target. Below about this a finger misses more than it hits.
const TOUCH_TARGET: f32 = 44.0;

/// The row of controls, hidden unless the layout is compact.
#[derive(Component)]
pub struct TouchBar;

/// What a button does when tapped.
///
/// An enum rather than a closure per button so the observer is written once and
/// the bar stays a list of labels and intents.
#[derive(Clone, Copy, Component)]
pub enum TouchAction {
    PreviousSeed,
    NextSeed,
    RollSeed,
    FewerFloors,
    MoreFloors,
    ToggleRegions,
}

impl TouchAction {
    const fn label(self) -> &'static str {
        // ASCII only: the tool ships no font asset, so anything outside Bevy's
        // default subset renders as a blank box.
        match self {
            Self::PreviousSeed => "< SEED",
            Self::NextSeed => "SEED >",
            Self::RollSeed => "ROLL",
            Self::FewerFloors => "- FLOOR",
            Self::MoreFloors => "+ FLOOR",
            Self::ToggleRegions => "REGIONS",
        }
    }

    const ALL: [Self; 6] = [
        Self::PreviousSeed,
        Self::NextSeed,
        Self::RollSeed,
        Self::FewerFloors,
        Self::MoreFloors,
        Self::ToggleRegions,
    ];
}

/// Apply a tapped control. The keyboard path calls these same methods.
pub fn apply_touch_action(
    click: On<Pointer<Click>>,
    actions: Query<&TouchAction>,
    time: Res<Time>,
    mut state: ResMut<crate::StudioState>,
) {
    let Ok(action) = actions.get(click.entity) else {
        return;
    };
    let now = time.elapsed_secs();
    match action {
        TouchAction::PreviousSeed => state.step_preset_seed(-1, now),
        TouchAction::NextSeed => state.step_preset_seed(1, now),
        TouchAction::RollSeed => state.roll_seed(now),
        TouchAction::FewerFloors => {
            let next = state.config.levels.saturating_sub(1);
            state.set_levels(next, now);
        }
        TouchAction::MoreFloors => {
            let next = state.config.levels.saturating_add(1);
            state.set_levels(next, now);
        }
        // The frontier overlay is the one thing here that is a *reading* aid
        // rather than a way to move around the seed space, and it is on the bar
        // for the same reason as the rest: the keyboard path does not exist on
        // a handset.
        TouchAction::ToggleRegions => {
            state.show_regions = !state.show_regions;
            state.touch_view();
        }
    }
}

/// Build the bar. One node per control, wrapping so it fits a narrow window.
pub fn spawn_touch_bar(column: &mut ChildSpawnerCommands, background: Color) {
    column
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(6.0),
                row_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(8.0)),
                justify_content: JustifyContent::Center,
                ..default()
            },
            Pickable::IGNORE,
            TouchBar,
            CompactOnly,
        ))
        .with_children(|row| {
            for action in TouchAction::ALL {
                row.spawn((
                    Node {
                        min_height: Val::Px(TOUCH_TARGET),
                        flex_grow: 1.0,
                        padding: UiRect::axes(Val::Px(12.0), Val::Px(10.0)),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(background),
                    BorderColor::all(schematic(SchematicRole::Selected).base_color),
                    action,
                ))
                .observe(apply_touch_action)
                .with_children(|button| {
                    button.spawn((
                        Text::new(action.label()),
                        TextFont {
                            font_size: FontSize::Px(13.0),
                            ..default()
                        },
                        TextColor(typography::Role::Heading.colour()),
                    ));
                });
            }
        });
}
