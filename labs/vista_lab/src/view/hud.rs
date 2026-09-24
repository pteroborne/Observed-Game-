//! The HUD: where you are, what is under you, and a legend for every lit thing.
//!
//! The Legibility Contract asks that no coloured signal go unexplained, so the legend
//! draws each swatch from the same style call the scene uses — it cannot drift from
//! what is on screen.
use bevy::prelude::*;
use observed_facility::hex_wfc::HexFace;
use observed_style::open_air::{SurveyRole, survey};
use observed_style::{MarkerRole, SurfaceRole, Treatment, marker, surface};

use super::{Lab, ViewMode, VistaEntity, cell_at};
use crate::exposure::{Drop, Form};

#[derive(Component)]
pub(super) struct Status;

#[derive(Component)]
pub(super) struct Legend;

#[derive(Component)]
pub(super) struct SurveyLegend;

const TEXT: Color = Color::srgb(0.90, 0.93, 0.96);
const PANEL: Color = Color::srgba(0.01, 0.015, 0.025, 0.72);

fn swatch(treatment: Treatment) -> Color {
    treatment.edge.unwrap_or(treatment.base_color)
}

fn row(parent: &mut ChildSpawnerCommands, color: Color, label: &str) {
    parent
        .spawn(Node {
            column_gap: Val::Px(8.0),
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                Node {
                    width: Val::Px(14.0),
                    height: Val::Px(6.0),
                    ..default()
                },
                BackgroundColor(color),
            ));
            row.spawn((
                Text::new(label),
                TextFont {
                    font_size: FontSize::Px(13.0),
                    ..default()
                },
                TextColor(TEXT),
            ));
        });
}

pub(super) fn spawn(mut commands: Commands) {
    commands.spawn((
        Status,
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(14.0),
            ..default()
        },
        TextColor(TEXT),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(16.0),
            bottom: Val::Px(14.0),
            padding: UiRect::all(Val::Px(8.0)),
            ..default()
        },
        BackgroundColor(PANEL),
    ));
    let panel = || Node {
        position_type: PositionType::Absolute,
        right: Val::Px(16.0),
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(4.0),
        padding: UiRect::all(Val::Px(8.0)),
        ..default()
    };
    commands
        .spawn((
            Legend,
            Node {
                top: Val::Px(14.0),
                ..panel()
            },
            BackgroundColor(PANEL),
        ))
        .with_children(|legend| {
            row(
                legend,
                swatch(surface(SurfaceRole::GantryEdge)),
                "lit lip: a drop — step past it and you fall",
            );
            row(
                legend,
                swatch(surface(SurfaceRole::GantryDeck)),
                "walkway deck: 2.6 m wide, air on both sides",
            );
            row(
                legend,
                swatch(marker(MarkerRole::Exit)),
                "green beam: the summit",
            );
            row(
                legend,
                swatch(observed_style::architecture_practical_fixture(
                    observed_content::ArchitectureRegister::Monolith,
                )),
                "window slits: lit rooms (architecture, not a signal)",
            );
        });
    commands
        .spawn((
            SurveyLegend,
            Node {
                top: Val::Px(128.0),
                ..panel()
            },
            BackgroundColor(PANEL),
            Visibility::Hidden,
        ))
        .with_children(|legend| {
            for role in SurveyRole::ALL {
                row(legend, survey(role), role.label());
            }
        });
}

type HudQueries<'w, 's> = (
    Query<'w, 's, (&'static mut Text, &'static mut Visibility), With<Status>>,
    Query<'w, 's, &'static mut Visibility, (With<Legend>, Without<Status>)>,
    Query<'w, 's, &'static mut Visibility, (With<SurveyLegend>, Without<Status>, Without<Legend>)>,
);

pub(super) fn update(
    time: Res<Time>,
    capture: Option<Res<crate::capture::Capture>>,
    mut lab: ResMut<Lab>,
    entities: Query<(), With<VistaEntity>>,
    (mut status, mut legend, mut survey_legend): HudQueries,
) {
    if let Some((_, left)) = lab.note.as_mut() {
        *left -= time.delta_secs();
    }
    if lab.note.as_ref().is_some_and(|(_, left)| *left <= 0.0) {
        lab.note = None;
    }
    let show = lab.hud && capture.is_none();
    let visibility = |on: bool| {
        if on {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        }
    };
    for mut v in &mut legend {
        *v = visibility(show);
    }
    for mut v in &mut survey_legend {
        *v = visibility(show && lab.survey_overlay);
    }
    let Ok((mut text, mut v)) = status.single_mut() else {
        return;
    };
    *v = visibility(show);
    if !show {
        return;
    }
    let mut lines = Vec::new();
    let mode = match lab.mode {
        ViewMode::Walk if lab.touring => "TOUR",
        ViewMode::Walk => "WALK",
        ViewMode::Fly => "FLY",
    };
    let (eye, _) = lab.eye();
    let feet = lab.walker.feet();
    let probe = if lab.mode == ViewMode::Walk {
        feet + Vec3::Y * 0.2
    } else {
        eye
    };
    match cell_at(&lab.vista, probe).and_then(|at| {
        lab.exposures
            .iter()
            .find(|e| e.coord == at)
            .map(|e| (at, *e))
    }) {
        Some((at, e)) => {
            let form = match e.form {
                Form::Deck => "deck",
                Form::Pavilion => "pavilion",
                Form::Storey => "storey",
                Form::Span { .. } => "walkway",
                Form::Flight { .. } => "stair flight",
                Form::Landing => "flight head",
            };
            let landmark = lab
                .vista
                .landmarks
                .get(&at)
                .map_or("", |landmark| landmark.label());
            let rails = if e.railed { "railed" } else { "no rails" };
            lines.push(format!(
                "{mode} · {landmark} · {form} · {} of 6 faces open to air · {rails}",
                HexFace::LATERAL
                    .into_iter()
                    .filter(|&f| e.is_sheer(f))
                    .count()
            ));
            let below = match (e.form, e.drop) {
                (_, Drop::Supported) => "Below: structure".to_string(),
                (_, Drop::Hanging { onto: None, .. }) => {
                    "Below: open air, then true void".to_string()
                }
                (
                    _,
                    Drop::Hanging {
                        onto: Some(onto), ..
                    },
                ) => format!(
                    "Below: {:.0} m of air to {}",
                    e.drop.metres().unwrap_or(0.0),
                    lab.vista
                        .landmarks
                        .get(&onto)
                        .map_or("structure", |landmark| landmark.label())
                ),
            };
            lines.push(below);
        }
        None => lines.push(format!("{mode} · in open air · {:.0} m up", eye.y)),
    }
    if let Some((note, _)) = &lab.note {
        lines.push(note.clone());
    }
    lines.push(format!(
        "air cells {} · pieces {} · lab entities {} · falls {}",
        lab.vista.air_cells,
        lab.build.pieces.len(),
        entities.iter().count(),
        lab.falls
    ));
    lines.push(
        "[1-7] vantages  [T] tour  [F] fly  [R] Bastion  [Backspace] rebuild  [F1] HUD  [F3] survey"
            .to_string(),
    );
    text.0 = lines.join("\n");
}
