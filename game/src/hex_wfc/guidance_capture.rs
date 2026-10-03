//! Private evidence fixtures for in-match UX. Each invokes the rules or production
//! camera/map input adapter; it never runs in an ordinary game.
use bevy::input::{
    ButtonState,
    keyboard::{Key, KeyboardInput},
};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

#[derive(Clone, Copy)]
pub(crate) enum Case {
    Human,
    Large,
    Dark,
    Empty,
    Rescue,
    Jailed,
    Rogue,
    Map,
    Guide,
    Overview,
    Eyes,
    Follow,
    Review,
}

pub(crate) fn stage(world: &mut World, case: Case) {
    use crate::hex_wfc::{sim::HexWfcRuntime, view::spectate};
    use Case::*;
    if matches!(case, Review) {
        let entity = world
            .query::<(Entity, &crate::screens::widgets::WidgetId)>()
            .iter(world)
            .find(|(_, id)| **id == crate::screens::widgets::WidgetId::named("hex.pause.role_help"))
            .expect("production pause menu role-help action")
            .0;
        world.trigger(bevy::ui_widgets::Activate { entity });
        return;
    }
    if matches!(case, Human | Rogue) {
        world
            .resource_mut::<crate::settings::Settings>()
            .gameplay_text_scale = 1.0;
    }
    if matches!(case, Large) {
        world
            .resource_mut::<crate::settings::Settings>()
            .gameplay_text_scale = 1.25;
    }
    {
        let mut runtime = world.resource_mut::<HexWfcRuntime>();
        let local = runtime.local_player;
        let floor = runtime.local().cell.level;
        match case {
            Dark => runtime
                .ascent
                .as_mut()
                .expect("Ascent fixture")
                .stage_power(floor, false),
            Empty => {
                let ascent = runtime.ascent.as_mut().expect("Ascent fixture");
                ascent.stage_power(floor, true);
                ascent.stage_charge(local, 0);
            }
            Rescue => {
                let team = runtime.local().team;
                let teammate = runtime
                    .match_state
                    .players
                    .values()
                    .find(|player| player.id != local && player.team == team)
                    .expect("Team fixture has a teammate")
                    .id;
                runtime.match_state.jail(teammate);
            }
            Jailed => {
                runtime.match_state.jail(local);
            }
            Rogue => {
                runtime.match_state.drop_into_void(local);
            }
            _ => {}
        }
    }
    let key = match case {
        Guide => Some(KeyCode::KeyH),
        Overview => Some(spectate::TOGGLE_KEY),
        Eyes => Some(spectate::EYES_KEY),
        Follow => Some(spectate::CYCLE_KEY),
        _ => None,
    };
    if let Some(key) = key {
        let window = world
            .query_filtered::<Entity, With<PrimaryWindow>>()
            .single(world)
            .expect("capture window");
        let character = match key {
            KeyCode::KeyH => "h",
            KeyCode::KeyO => "o",
            KeyCode::KeyV => "v",
            KeyCode::KeyF => "f",
            _ => unreachable!("capture key"),
        };
        // Queue both edges through Bevy's normal input messages. PreUpdate consumes
        // them before the ordinary match systems; no renderer runs out of schedule.
        for state in [ButtonState::Pressed, ButtonState::Released] {
            world.write_message(KeyboardInput {
                key_code: key,
                logical_key: Key::Character(character.into()),
                state,
                text: (state == ButtonState::Pressed).then(|| character.into()),
                repeat: false,
                window,
            });
        }
    }
}
