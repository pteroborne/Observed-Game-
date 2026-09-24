//! Materials for open edges: the lit lip, the railing, the walkway and its truss.
//!
//! The same in every register. The lip is a gameplay signal, and the railing, walkway
//! and truss belong to the connective structure between districts rather than to any
//! one of them.

use bevy::prelude::*;
use observed_content::ArchitectureRegister;
use observed_style::{self as style, SurfaceRole};

#[derive(Clone)]
pub(super) struct OpenEdgeMaterials {
    pub(super) lip: Handle<StandardMaterial>,
    pub(super) rail: Handle<StandardMaterial>,
    pub(super) walkway: Handle<StandardMaterial>,
    pub(super) truss: Handle<StandardMaterial>,
}

impl OpenEdgeMaterials {
    pub(super) fn new(materials: &mut Assets<StandardMaterial>) -> Self {
        // A signal is lit, not `unlit`: Bevy's unlit path drops emission, and the
        // fall edge's albedo is dark on purpose. The glow is the signal.
        let lip = style::surface(SurfaceRole::GantryEdge);
        let shell = |treatment: style::Treatment| {
            let look = style::hex_shell_look(&treatment, ArchitectureRegister::Megastructure);
            StandardMaterial {
                base_color: look.base_color,
                emissive: look.emissive,
                perceptual_roughness: 0.8,
                ..default()
            }
        };
        Self {
            lip: materials.add(StandardMaterial {
                base_color: lip.base_color,
                emissive: lip.emissive,
                perceptual_roughness: 0.4,
                ..default()
            }),
            rail: materials.add(shell(style::surface(SurfaceRole::Wall))),
            walkway: materials.add(shell(style::surface(SurfaceRole::GantryDeck))),
            truss: materials.add(StandardMaterial {
                base_color: style::open_air::sky(style::open_air::SkyRole::Underside),
                perceptual_roughness: 0.95,
                ..default()
            }),
        }
    }
}
