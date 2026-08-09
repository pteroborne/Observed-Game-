//! Bevy UI panel chrome, tab routing, and the status line.
//!
//! Keyboard handling lives in [`crate::input`].

use bevy::prelude::*;
use observed_content::ArchitectureRegister;
use observed_style::{SchematicRole, schematic, schematic_screen};

use crate::panels::score::format_score_breakdown;
use crate::panels::tuning::format_tuning_panel;
use crate::persist::simulation_hash;
use crate::pick::diagnostics;

#[derive(Component)]
pub struct ChromeMenuRoot;

#[derive(Component)]
pub struct ChromeMenuText;

#[derive(Component)]
pub struct ChromeStatusText;

/// The always-on "what will my next click do" block.
#[derive(Component)]
pub struct ChromeActionText;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StudioTab {
    Solve,
    Tuning,
    Pins,
    Coverage,
    Neighbours,
    Districts,
    Diagnostics,
}

impl StudioTab {
    pub const ALL: [Self; 7] = [
        Self::Solve,
        Self::Tuning,
        Self::Pins,
        Self::Coverage,
        Self::Neighbours,
        Self::Districts,
        Self::Diagnostics,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Solve => "SOLVE",
            Self::Tuning => "TUNING",
            Self::Pins => "PINS",
            Self::Coverage => "COVERAGE",
            Self::Neighbours => "NEIGHBOURS",
            Self::Districts => "DISTRICTS",
            Self::Diagnostics => "DIAGNOSTICS",
        }
    }
}

#[derive(Resource)]
pub struct LabMenuState {
    pub is_open: bool,
    pub active_tab: usize,
    pub selected_item: usize,
    /// A modal confirmation waiting on Enter/Esc. While set it owns the
    /// keyboard completely — promoting a profile changes the value every LAN
    /// peer must match, so it must not be one keystroke away from a slider.
    pub confirm: Option<PendingConfirm>,
}

/// The only irreversible action this tool has.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingConfirm {
    pub prompt: String,
}

impl Default for LabMenuState {
    fn default() -> Self {
        Self {
            is_open: true,
            active_tab: 1, // Default to Tuning tab
            selected_item: 0,
            confirm: None,
        }
    }
}

impl LabMenuState {
    pub fn tab(&self) -> StudioTab {
        StudioTab::ALL[self.active_tab % StudioTab::ALL.len()]
    }

    pub fn next_tab(&mut self) {
        self.active_tab = (self.active_tab + 1) % StudioTab::ALL.len();
        self.selected_item = 0;
    }

    pub fn prev_tab(&mut self) {
        self.active_tab = (self.active_tab + StudioTab::ALL.len() - 1) % StudioTab::ALL.len();
        self.selected_item = 0;
    }

    /// Jump to a named tab, for a key that opens a view and its readout
    /// together. Leaving the panel on TUNING while the viewport switched to
    /// something else is how a mode gets called broken.
    pub fn set_tab(&mut self, tab: StudioTab) {
        if let Some(index) = StudioTab::ALL.iter().position(|entry| *entry == tab) {
            self.active_tab = index;
            self.selected_item = 0;
        }
    }
}

/// Setup Bevy UI overlay nodes.
pub fn setup_chrome(mut commands: Commands) {
    let screen_bg = schematic_screen();
    let mat_bg = screen_bg.with_alpha(0.86);

    // Root UI container
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                // Row, not column: the panel is a sibling of the viewport area
                // now, not an overlay floating above it.
                flex_direction: FlexDirection::Row,
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|parent| {
            // Docked panel: a full-height column on the left. The 3D camera's
            // viewport is inset by exactly this width, so the panel sits beside
            // the facility rather than over it and nothing is ever hidden.
            parent
                .spawn((
                    Node {
                        width: Val::Px(crate::PANEL_WIDTH),
                        height: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        // Content hugs the left edge. Without this the text node
                        // is content-sized and drifts to the far side of the
                        // panel, which reads as a rendering fault rather than a
                        // layout default.
                        align_items: AlignItems::FlexStart,
                        padding: UiRect::all(Val::Px(16.0)),
                        border: UiRect::right(Val::Px(2.0)),
                        overflow: Overflow::scroll_y(),
                        ..default()
                    },
                    BackgroundColor(mat_bg),
                    // Recoloured each frame to show which region owns the
                    // keyboard, so "where do my keys go" is never a guess.
                    BorderColor::all(Color::NONE),
                    ChromeMenuRoot,
                ))
                .with_children(|menu| {
                    menu.spawn((
                        // Explicit full width. A `Text` entity's implicit node is
                        // content-sized, and a content-sized box inside a column
                        // gets placed by the cross-axis rule rather than pinned to
                        // the padding edge - which reads as the panel rendering in
                        // the wrong place. Sizing the box to the panel makes both
                        // the justification below and the wrap width mean what
                        // they say.
                        Node {
                            width: Val::Percent(100.0),
                            ..default()
                        },
                        Text::new("COMPOSITION STUDIO"),
                        TextFont {
                            font_size: 14.0,
                            ..default()
                        },
                        // Monospace panel content is laid out with spaces; centre
                        // justification would break every column alignment in it.
                        TextLayout::new_with_justify(Justify::Left),
                        TextColor(schematic(SchematicRole::Selected).base_color),
                        ChromeMenuText,
                    ));
                    // Real controls for every tab that declares tunables, as
                    // siblings after the text node: on those tabs the text
                    // above holds only the header and these rows are the body.
                    crate::field_widgets::spawn_field_rows(menu);
                });

            // The viewport column: everything to the right of the panel. Its
            // chrome (action bar, status line) sits at the bottom so the
            // facility itself keeps the whole upper area.
            parent
                .spawn((
                    Node {
                        flex_grow: 1.0,
                        height: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::FlexEnd,
                        ..default()
                    },
                    Pickable::IGNORE,
                ))
                .with_children(|column| {
                    column
                        .spawn((
                            Node {
                                width: Val::Px(430.0),
                                margin: UiRect::left(Val::Px(16.0)),
                                padding: UiRect::all(Val::Px(12.0)),
                                flex_direction: FlexDirection::Column,
                                ..default()
                            },
                            BackgroundColor(mat_bg),
                        ))
                        .with_children(|bar| {
                            bar.spawn((
                                Text::new(""),
                                TextFont {
                                    font_size: 13.0,
                                    ..default()
                                },
                                TextLayout::new_with_justify(Justify::Left),
                                TextColor(schematic(SchematicRole::Pinned).base_color),
                                ChromeActionText,
                            ));
                        });

                    column
                        .spawn((
                            Node {
                                width: Val::Percent(100.0),
                                min_height: Val::Px(40.0),
                                align_items: AlignItems::Center,
                                padding: UiRect::axes(Val::Px(16.0), Val::Px(8.0)),
                                ..default()
                            },
                            BackgroundColor(mat_bg),
                        ))
                        .with_children(|bar| {
                            bar.spawn((
                                Text::new("Status Line"),
                                TextFont {
                                    font_size: 13.0,
                                    ..default()
                                },
                                TextLayout::new_with_justify(Justify::Left),
                                TextColor(schematic(SchematicRole::Pinned).base_color),
                                ChromeStatusText,
                            ));
                        });
                });
        });
}

/// Repaint the panel and the status line from current state.
#[allow(clippy::type_complexity)] // Three disjoint text queries in one system.
pub fn update_chrome_ui(
    menu_state: Res<LabMenuState>,
    state: Res<crate::StudioState>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut menu_query: Query<
        &mut Text,
        (
            With<ChromeMenuText>,
            Without<ChromeStatusText>,
            Without<ChromeActionText>,
        ),
    >,
    mut status_query: Query<
        &mut Text,
        (
            With<ChromeStatusText>,
            Without<ChromeMenuText>,
            Without<ChromeActionText>,
        ),
    >,
    mut action_query: Query<
        &mut Text,
        (
            With<ChromeActionText>,
            Without<ChromeMenuText>,
            Without<ChromeStatusText>,
        ),
    >,
    mut root_query: Query<(&mut Node, &mut BorderColor), With<ChromeMenuRoot>>,
) {
    if let Ok(mut action) = action_query.single_mut() {
        let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
        let ctrl =
            keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);
        **action = crate::actionbar::format_action_bar(&state, &menu_state, shift, ctrl);
    }

    if let Ok((mut node, mut border)) = root_query.single_mut() {
        // `Display::None` rather than `Visibility::Hidden`: a hidden node still
        // occupies its width, and the camera viewport is inset by that width,
        // so the facility would keep a blank stripe beside it.
        node.display = if state.panel_open {
            Display::Flex
        } else {
            Display::None
        };
        // The owned region carries a lit border. Focus that relies on nothing
        // visible is focus you have to discover by experiment.
        *border = BorderColor::all(match state.keyboard_owner {
            crate::KeyboardOwner::Panel => schematic(SchematicRole::Selected).base_color,
            crate::KeyboardOwner::Viewport => Color::NONE,
        });
    }

    if state.panel_open {
        let Ok(mut text) = menu_query.single_mut() else {
            return;
        };
        let mut tabs_header = String::new();
        for tab in StudioTab::ALL {
            if tab == menu_state.tab() {
                tabs_header.push_str(&format!("[ {} ] ", tab.label()));
            } else {
                tabs_header.push_str(&format!("  {}   ", tab.label()));
            }
        }

        let tab_content = match menu_state.tab() {
            StudioTab::Solve => {
                let mut s = String::new();
                s.push_str("SOLVE CONTROLS & A/B COMPARISON:\n\n");
                s.push_str(&format!(
                    "Seed Index: {} (Preset Seed: {:#x})\n",
                    state.seed_index,
                    crate::PRESET_SEEDS[state.seed_index % crate::PRESET_SEEDS.len()]
                ));
                s.push_str(&format!(
                    "Working Scale: {}x{}x{}\n",
                    state.config.cols, state.config.rows, state.config.levels
                ));
                s.push('\n');

                if let Some(solved) = state.solved.as_ref() {
                    // The ladder first: it explains the score below it, and it
                    // is empty unless a search actually ran.
                    s.push_str(&crate::panels::search::format_candidate_ladder(
                        &solved.candidates,
                    ));
                    s.push_str(&format_score_breakdown(
                        &solved.score,
                        state.baseline_score.as_ref(),
                    ));
                } else {
                    s.push_str("No solve result available.");
                }
                s
            }
            StudioTab::Tuning => format_tuning_panel(&state),
            StudioTab::Pins => crate::panels::pins::format_pins_panel(&state),
            StudioTab::Coverage => match state.solved.as_ref() {
                Some(solved) => crate::panels::coverage::format_coverage_panel(
                    &solved.coverage,
                    &state.seam_audit,
                ),
                None => String::from("No solve result available."),
            },
            StudioTab::Neighbours => {
                crate::panels::neighbors::format_neighbors_panel(&state.neighbors)
            }
            StudioTab::Districts => {
                let mut s = String::new();
                s.push_str("DISTRICT BIAS OVERRIDES:\n\n");
                for register in ArchitectureRegister::ALL {
                    s.push_str(&format!(
                        "{:<16}: room={:.2} straight={:.2} shaft={:.2} expanse={:.2}\n",
                        register.slug(),
                        state.profile.district_bias_for(
                            register,
                            observed_facility::hex_wfc::HexArchetype::Room
                        ),
                        state.profile.district_bias_for(
                            register,
                            observed_facility::hex_wfc::HexArchetype::Straight
                        ),
                        state.profile.district_bias_for(
                            register,
                            observed_facility::hex_wfc::HexArchetype::Shaft
                        ),
                        state.profile.district_bias_for(
                            register,
                            observed_facility::hex_wfc::HexArchetype::Expanse
                        ),
                    ));
                }
                s
            }
            StudioTab::Diagnostics => {
                let mut s = String::new();
                s.push_str("DIAGNOSTICS & CELL INSPECTION:\n\n");
                s.push_str(&diagnostics(&state));

                if let Err(defects) = state.profile.validate() {
                    s.push_str("\n\nPROFILE DEFECTS:\n");
                    for d in defects {
                        s.push_str(&format!("- {d}\n"));
                    }
                }
                s
            }
        };

        // A pending confirmation replaces the panel entirely: there must be
        // nothing else to look at while an irreversible action is asked.
        **text = match menu_state.confirm.as_ref() {
            Some(confirm) => confirm.prompt.clone(),
            None => format!("{tabs_header}\n\n{tab_content}"),
        };
    }

    if let Ok(mut status) = status_query.single_mut() {
        let unsaved = if state.is_unsaved() { " *UNSAVED" } else { "" };
        // The hash a LAN peer must match. `simulation_hash` reports
        // "unavailable" rather than inventing a digest it could not compute.
        let sim = simulation_hash(&state, &state.profile);
        // Naming the mode is not optional: the same two colours answer two
        // different questions, so the legend has to travel with the view.
        let compare = if state.detail_mode == crate::detail::DetailMode::Neighborhood {
            // The schematic has stepped back to Grid here, so the only green
            // and red on screen are the ring's. Saying so is the same rule the
            // other two modes follow: the legend travels with the view.
            let report = &state.neighbors.report;
            format!(
                " | NEIGHBOURS (green = as solved, red = alternative): \
                 {} of {} cage(s) altered{}",
                report.altered,
                report.ring_cells,
                if report.unbuildable > 0 {
                    format!(", {} WITH NO GEOMETRY", report.unbuildable)
                } else {
                    String::new()
                }
            )
        } else if state.show_baseline_compare {
            format!(
                " | COMPARE (red = moved): {} of {} differ",
                state.report.changed, state.report.cells
            )
        } else {
            format!(
                " | PINS (green = held): {} pinned, brush {}",
                state.report.pinned,
                state.brush.label()
            )
        };
        let defects = state
            .pin_diagnostics
            .iter()
            .filter(|d| {
                !matches!(
                    d,
                    observed_facility::hex_wfc::PinDiagnostic::BlueprintCollision { .. }
                        | observed_facility::hex_wfc::PinDiagnostic::Shadowed { .. }
                )
            })
            .count();
        let compare = if defects > 0 {
            format!("{compare} | {defects} PIN ERROR(S)")
        } else {
            compare
        };

        let detail = if state.detail_mode == crate::detail::DetailMode::Off {
            String::new()
        } else {
            format!(
                " | {} view {}/{}{}: {} cell(s), {} hull(s), {} cut, {} cached",
                state.detail_mode.label(),
                state.detent + 1,
                crate::viewport::AZIMUTH_DETENTS,
                if state.cutaway { "" } else { " (no cutaway)" },
                state.detail_report.cells,
                state.detail_report.hulls_drawn,
                state.detail_report.hulls_cut,
                state.detail_report.distinct_hulls_cached,
            )
        };

        **status = format!(
            "F2 menu | sim {sim} | {}{unsaved} | {} | {}{compare}{detail} | {}",
            state.profile.label,
            state.layer.label(),
            format_args!("{} cells", state.report.cells),
            state.status,
        );
    }
}
