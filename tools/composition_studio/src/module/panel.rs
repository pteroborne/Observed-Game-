//! The text half: which module, what is wrong with it, and what to press.

use bevy::prelude::*;
use observed_style::{SchematicRole, schematic, schematic_screen};

use crate::module::app::ModuleState;

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
    mut panel: Query<&mut Text, (With<ModulePanelText>, Without<ModuleStatusText>)>,
    mut status: Query<&mut Text, (With<ModuleStatusText>, Without<ModulePanelText>)>,
) {
    if let Ok(mut text) = panel.single_mut() {
        **text = format_panel(&state);
    }
    if let Ok(mut text) = status.single_mut() {
        **text = format!(
            "{}  |  [Up/Dn] module  [Tab] next failing  [Q/E] orbit  wheel zoom",
            state.status
        );
    }
}

/// The panel body. Pure so it can be asserted on without an app.
#[must_use]
pub fn format_panel(state: &ModuleState) -> String {
    let Some(diagnosis) = state.current() else {
        return String::from(
            "MODULE STUDIO\n\nNo authored modules found.\n\n\
             Run from the repo root, or pass a directory:\n\
             cargo run -p composition_studio --bin module-studio -- <dir>",
        );
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
