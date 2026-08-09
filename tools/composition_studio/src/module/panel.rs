//! The text half: which module, what is wrong with it, and what to press.

use bevy::prelude::*;
use observed_style::{SchematicRole, schematic, schematic_screen};

use crate::module::app::ModuleState;
use crate::module::certify;
use crate::module::rapier_audit::RapierAuditState;

#[derive(Component)]
pub struct ModulePanelText;

#[derive(Component)]
pub struct ModuleStatusText;

pub fn setup_panel(mut commands: Commands) {
    let background = schematic_screen().with_alpha(0.86);
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::all(Val::Px(16.0)),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Px(560.0),
                    padding: UiRect::all(Val::Px(12.0)),
                    ..default()
                },
                BackgroundColor(background),
                Text::new(""),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextLayout::new_with_justify(Justify::Left),
                TextColor(schematic(SchematicRole::Selected).base_color),
                ModulePanelText,
            ));
            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(background),
                Text::new(""),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
                TextLayout::new_with_justify(Justify::Left),
                TextColor(schematic(SchematicRole::Pinned).base_color),
                ModuleStatusText,
            ));
        });
}

#[allow(clippy::type_complexity)]
pub fn update_panel(
    state: Res<ModuleState>,
    view: Res<crate::module::neighbor_view::NeighbourView>,
    mut panel: Query<&mut Text, (With<ModulePanelText>, Without<ModuleStatusText>)>,
    mut status: Query<&mut Text, (With<ModuleStatusText>, Without<ModulePanelText>)>,
) {
    if let Ok(mut text) = panel.single_mut() {
        // In the ring mode the neighbours block replaces the diagnostic tail
        // rather than being appended to it. The panel does not scroll, so
        // appending would push the answer off the bottom of the screen.
        **text = if view.on {
            format!(
                "{}\n{}",
                format_header(&state),
                crate::module::neighbor_panel::format_neighbours(&view)
            )
        } else {
            format_panel(&state)
        };
    }
    if let Ok(mut text) = status.single_mut() {
        let keys = if view.on {
            "[N] neighbours off  [1-9] cell  [,/.] cycle  [Space] roll  [Shift+Space] reset  [Q/E] orbit"
        } else {
            "[Up/Dn] module  [Tab] next failing  [N] neighbours  [R] Rapier audit  [C] cutaway  [Q/E] orbit"
        };
        let summary = if view.on {
            format!("  |  {}", crate::module::neighbor_panel::headline(&view))
        } else {
            String::new()
        };
        **text = format!("{}{summary}  |  {keys}", state.status);
    }
}

/// Shown when the watched directory holds nothing to look at.
const EMPTY_CORPUS: &str = "MODULE STUDIO\n\nNo authored modules found.\n\n\
     Run from the repo root, or pass a directory:\n\
     cargo run -p composition_studio --bin module-studio -- <dir>";

/// Name, shape and verdict: the part of the panel that is true in every mode.
///
/// Split out because the neighbour ring **replaces** the diagnostic tail rather
/// than appending to it - the panel does not scroll, so a mode that appended
/// would push its own answer off the bottom - but it must never replace this.
/// Which module you are looking at and whether it validates are the two facts
/// no view of it may drop.
#[must_use]
pub fn format_header(state: &ModuleState) -> String {
    let Some(diagnosis) = state.current() else {
        return String::from(EMPTY_CORPUS);
    };
    let mut lines = vec![
        String::from("MODULE STUDIO"),
        String::new(),
        format!(
            "{}  ({} of {})",
            diagnosis.name(),
            state.selected + 1,
            state.diagnoses.len()
        ),
    ];
    if let Some(summary) = diagnosis.summary.as_ref() {
        lines.push(format!(
            "{:?}  {} cell(s)  {} port(s)  {} socket(s)  {} hull(s)",
            summary.kind, summary.footprint_cells, summary.ports, summary.sockets, summary.hulls
        ));
    } else if let Some(prototype) = diagnosis.prototype.as_ref() {
        lines.push(format!(
            "{} hull(s)  {} level(s)  (unvalidated: counts from geometry only)",
            prototype.hulls.len(),
            prototype.levels
        ));
    }
    lines.push(match diagnosis.error.as_ref() {
        Some(_) => String::from("INVALID"),
        None => String::from("VALID"),
    });
    lines.join("\n")
}

/// The panel body. Pure so it can be asserted on without an app.
#[must_use]
pub fn format_panel(state: &ModuleState) -> String {
    let Some(diagnosis) = state.current() else {
        return String::from(EMPTY_CORPUS);
    };

    let mut lines = vec![
        String::from("MODULE STUDIO"),
        String::new(),
        format!(
            "{}  ({} of {})",
            diagnosis.name(),
            state.selected + 1,
            state.diagnoses.len()
        ),
    ];

    // The full summary needs a validated module. When validation failed, fall
    // back to what the geometry alone can say - the same principle as still
    // drawing the module: the shape is *most* worth knowing at the moment
    // something is wrong with it, so going blank there is backwards.
    if let Some(summary) = diagnosis.summary.as_ref() {
        lines.push(format!(
            "{:?}  {} cell(s)  {} port(s)  {} socket(s)  {} hull(s)",
            summary.kind, summary.footprint_cells, summary.ports, summary.sockets, summary.hulls
        ));
    } else if let Some(prototype) = diagnosis.prototype.as_ref() {
        lines.push(format!(
            "{} hull(s)  {} level(s)  (unvalidated: counts from geometry only)",
            prototype.hulls.len(),
            prototype.levels
        ));
    }

    // Never let hidden geometry read as absent geometry. A cutaway that
    // says nothing turns "I cut this away" and "this was never authored"
    // into the same picture.
    if state.cutaway {
        lines.push(if state.cut_hulls > 0 {
            format!(
                "CUTAWAY on - {} hull(s) hidden  [C] to show all",
                state.cut_hulls
            )
        } else {
            String::from("CUTAWAY on - nothing to hide at this angle")
        });
    }

    lines.push(String::new());
    match diagnosis.error.as_ref() {
        Some(error) => {
            lines.push(String::from("INVALID"));
            lines.push(String::new());
            for line in error.lines() {
                lines.push(line.to_string());
            }
            // Say when there is nothing to point at, rather than leaving the
            // author hunting the viewport for a highlight that was never drawn.
            if matches!(
                diagnosis.highlight,
                crate::module::diagnose::Highlight::Whole
            ) {
                lines.push(String::new());
                lines.push(String::from(
                    "(no single place to point at: this is a property, not a position)",
                ));
            }
        }
        None => {
            lines.push(String::from("VALID"));
            if diagnosis.prototype.is_none() {
                lines.push(String::from("...but nothing parsed as geometry."));
            }
        }
    }

    if let Some(guide) = state.guide.as_ref() {
        let climbs = guide.climb.as_ref().map_or(0, |spine| spine.nodes.len());
        let decks = guide.deck.as_ref().map_or(0, |deck| deck.nodes.len());
        lines.push(format!(
            "GUIDE  climb {climbs} node(s)  deck {decks} node(s)"
        ));
    } else {
        lines.push(String::from("GUIDE  no declared climb/deck"));
    }
    lines.push(String::from(
        "LEGEND  selected=guide  pinned=Rapier trace  grid=preflight  volatile=failure",
    ));

    // The walk verdict sits with the validity verdict, because they answer
    // different questions and a module can pass one and fail the other - which
    // is the entire reason this probe exists.
    if let Some(report) = state.walk.as_ref() {
        let limits = crate::module::walk::Thresholds::default();
        lines.push(String::new());
        match report.failure {
            None => lines.push(format!(
                "PREFLIGHT clear - {} samples, {:.1} m climbed",
                report.path.len(),
                report.climbed
            )),
            Some(failure) => {
                lines.push(format!(
                    "PREFLIGHT blocked at {:.0}%",
                    report.progress * 100.0
                ));
                lines.push(failure.describe(&limits));
            }
        }
        if let Some((clearance, _)) = report.tightest
            && clearance < limits.authoring_headroom_standard
        {
            lines.push(format!(
                "AUTHORING PINCH  {clearance:.2} m < {:.2} m standard",
                limits.authoring_headroom_standard
            ));
        }
    }

    lines.push(String::new());
    match &state.rapier_audit {
        RapierAuditState::Off => lines.push(String::from("RAPIER off  [R] run selected module")),
        RapierAuditState::NotApplicable => lines.push(String::from(
            "RAPIER not applicable - no declared climb/deck guide",
        )),
        RapierAuditState::Complete(report) => {
            lines.push(format!(
                "RAPIER {}  profile {}",
                if report.passed() { "PASS" } else { "FAIL" },
                report.profile_hash_prefix()
            ));
            for leg in &report.legs {
                let label = format!("{:?} {:?}", leg.kind, leg.direction);
                if let Some(failure) = leg.failure {
                    lines.push(format!(
                        "  {label}: {failure:?} at tick {} ({:?}) [{:.2}, {:.2}, {:.2}]",
                        leg.ticks,
                        leg.final_state,
                        leg.final_feet.x,
                        leg.final_feet.y,
                        leg.final_feet.z
                    ));
                } else {
                    lines.push(format!(
                        "  {label}: pass at tick {} ({:?})",
                        leg.ticks, leg.final_state
                    ));
                }
            }
        }
    }

    lines.push(certify::summary(&state.certification));

    if let Some(recipe) = diagnosis.recipe.as_ref() {
        lines.push(String::new());
        lines.push(format!("PARAMETRIC  {} step(s)", recipe.steps.len()));
        lines.push(String::from(
            "[S/X] step   [A/D] parameter   [Lt/Rt] adjust, Shift for 8x",
        ));
        // No save key, because there is nothing to save: a nudge writes the
        // recipe and the watcher re-previews it. Advertising a key that does
        // nothing is worse than advertising none.
        lines.push(String::from("every adjustment writes the recipe file"));
        lines.push(String::new());

        for (index, step) in recipe.steps.iter().enumerate() {
            let selected = index == state.step;
            let params = step.params();
            // Unselected rows carry the label only. The panel is a fixed-width
            // text node, and a full parameter list per row wraps mid-value into
            // an unreadable block - which is exactly what the first capture of
            // this showed.
            if !selected {
                lines.push(format!("  {:<16} {} param(s)", step.label(), params.len()));
                continue;
            }
            lines.push(format!("> {:<16} {} param(s)", step.label(), params.len()));
            for (slot, (name, value)) in params.iter().enumerate() {
                // The held parameter is bracketed rather than coloured: the
                // panel is one text node, and a reader scanning for "which one
                // moves" needs it legible without a colour key.
                let marker = if slot == state.param { ">" } else { " " };
                lines.push(format!("    {marker} {name:<14} {value:>9.2}"));
            }
        }
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::diagnose::{Diagnosis, Highlight};
    use crate::module::rapier_audit::{
        GuideLegKind, RapierAuditFailure, RapierAuditReport, RapierLegReport,
    };
    use observed_traversal::{FollowState, TraversalDirection};
    use std::path::PathBuf;

    fn state_with(error: Option<&str>, highlight: Highlight) -> ModuleState {
        ModuleState {
            diagnoses: vec![Diagnosis {
                path: PathBuf::from("assets/tiles/authored/hall_cap.map"),
                prototype: None,
                module: None,
                summary: None,
                error: error.map(ToString::to_string),
                highlight,
                recipe: None,
            }],
            ..ModuleState::default()
        }
    }

    /// The verdict has to be a word, not a colour: a capture, a colour-blind
    /// reader, and a grep all need it in the text.
    #[test]
    fn the_panel_states_the_verdict_in_words() {
        assert!(
            state_with(None, Highlight::Whole)
                .current()
                .is_some_and(Diagnosis::is_clean)
        );
        assert!(format_panel(&state_with(None, Highlight::Whole)).contains("VALID"));
        let failing = state_with(Some("Headroom { meters: 1.9 }"), Highlight::Whole);
        assert!(format_panel(&failing).contains("INVALID"));
        assert!(
            format_panel(&failing).contains("1.9"),
            "the numbers must survive"
        );
    }

    /// An unplaced error must say it is unplaced. Otherwise the author reads
    /// "INVALID", finds no highlight, and doubts the viewport rather than
    /// reading the message.
    #[test]
    fn an_unplaced_error_says_there_is_nothing_to_point_at() {
        let text = format_panel(&state_with(Some("MissingMeta"), Highlight::Whole));
        assert!(text.contains("no single place"), "{text}");

        let placed = format_panel(&state_with(Some("FloorGap"), Highlight::Vertex(Vec3::ZERO)));
        assert!(
            !placed.contains("no single place"),
            "a placed error must not claim otherwise: {placed}"
        );
    }

    /// An empty corpus is a normal state and must explain itself.
    #[test]
    fn an_empty_corpus_explains_how_to_point_the_tool_somewhere_else() {
        let text = format_panel(&ModuleState::default());
        assert!(text.contains("No authored modules"), "{text}");
        assert!(text.contains("module-studio"), "{text}");
    }

    /// The tool ships no font asset, same as its sibling.
    #[test]
    fn panel_text_stays_ascii() {
        for text in [
            format_panel(&ModuleState::default()),
            format_panel(&state_with(Some("Headroom"), Highlight::Whole)),
        ] {
            assert!(text.is_ascii(), "{text:?}");
        }
    }

    #[test]
    fn panel_keeps_preflight_and_rapier_verdicts_distinct() {
        let mut state = state_with(None, Highlight::Whole);
        state.walk = Some(crate::module::walk::WalkReport {
            path: vec![Vec3::ZERO, Vec3::X],
            failure: None,
            climbed: 0.0,
            progress: 1.0,
            tightest: Some((2.0, Vec3::ZERO)),
        });
        state.rapier_audit = RapierAuditState::Complete(RapierAuditReport {
            profile_hash: [0xabu8; 32],
            legs: vec![RapierLegReport {
                kind: GuideLegKind::Climb,
                direction: TraversalDirection::Forward,
                ticks: 90,
                final_state: FollowState::FollowingClimb,
                final_feet: Vec3::ZERO,
                last_target: Some(Vec3::X),
                trace: vec![Vec3::ZERO],
                failure: Some(RapierAuditFailure::TimedOut),
            }],
        });

        let text = format_panel(&state);
        assert!(text.contains("PREFLIGHT clear"), "{text}");
        assert!(text.contains("AUTHORING PINCH  2.00 m < 2.20 m"), "{text}");
        assert!(text.contains("RAPIER FAIL"), "{text}");
        assert!(text.contains("TimedOut at tick 90"), "{text}");
        assert!(text.contains("selected=guide"), "{text}");
        assert!(
            !text.contains("cursor"),
            "stateless state is not a graph cursor"
        );
        assert!(!text.contains("binding"), "no binding contract exists yet");
    }

    /// A parametric module must show its parameters, and must not advertise a
    /// key that does nothing. The first cut offered "[Ctrl+S] save" while every
    /// nudge already wrote the file.
    #[test]
    fn a_recipe_panel_shows_its_held_parameter_and_no_dead_keys() {
        use observed_authoring::forge::recipe::Recipe;

        let mut state = state_with(None, Highlight::Whole);
        state.diagnoses[0].recipe = Some(Recipe::starter("authored/panel_probe"));
        state.step = 0;
        state.param = 2;

        let text = format_panel(&state);
        assert!(text.contains("PARAMETRIC"), "{text}");
        assert!(
            text.contains("chamfer_top"),
            "the held step must list params"
        );
        assert!(
            !text.contains("Ctrl+S"),
            "no save key: a nudge already writes the file"
        );
        assert!(
            text.contains("writes the recipe file"),
            "the save behaviour must be stated: {text}"
        );
    }

    /// Only the held step lists its parameters. Every step listing everything
    /// wraps the fixed-width panel into an unreadable block.
    #[test]
    fn unheld_steps_stay_one_line() {
        use observed_authoring::forge::recipe::Recipe;

        let mut state = state_with(None, Highlight::Whole);
        state.diagnoses[0].recipe = Some(Recipe::starter("authored/panel_probe"));
        state.step = 0;

        let text = format_panel(&state);
        // The starter's second step is a hex_slab too; its params must not show.
        let listed = text.matches("chamfer_bottom").count();
        assert_eq!(listed, 1, "only the held step lists parameters: {text}");
    }
}
