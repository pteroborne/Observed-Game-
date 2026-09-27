//! What the prompt says at a floor's generator and recharge station, and whether the
//! local body's floor has power: the words `hud::play` shows for the fixtures.

use observed_match::ascent::economy::MAX_CHARGE;
use observed_match::ascent::facility::{AtFixture, Fixture};

use crate::hex_wfc::hud::words::PromptView;
use crate::hex_wfc::sim::HexWfcRuntime;
use crate::settings::{Settings, key_name};

/// The fixture the local body stands at, and what it would do for it now.
fn at_fixture(runtime: &HexWfcRuntime) -> Option<(Fixture, AtFixture)> {
    runtime
        .ascent
        .as_ref()?
        .at_fixture(&runtime.match_state, runtime.local_player)
}

/// The prompt the fixture the local body stands at asks for, if it stands at one.
pub(in crate::hex_wfc) fn prompt(
    runtime: &HexWfcRuntime,
    settings: &Settings,
) -> Option<PromptView> {
    let (_, at) = at_fixture(runtime)?;
    Some(prompt_for(at, settings))
}

/// What the prompt says at a fixture.
pub(in crate::hex_wfc) fn prompt_for(at: AtFixture, settings: &Settings) -> PromptView {
    let interact = || key_name(settings.bindings.interact);
    match at {
        AtFixture::Generator {
            powered: true,
            operable: true,
        } => PromptView {
            key: interact(),
            pad: "X",
            title: "Cut the floor's power".to_owned(),
            detail: "Its stations and doors go dead, and sight falls short",
            progress: None,
        },
        AtFixture::Generator {
            powered: false,
            operable: true,
        } => PromptView {
            key: interact(),
            pad: "X",
            title: "Restore the floor's power".to_owned(),
            detail: "Its stations charge again, and sight reaches",
            progress: None,
        },
        AtFixture::Generator { powered, .. } => PromptView {
            key: String::new(),
            pad: "",
            title: if powered {
                "Generator running".to_owned()
            } else {
                "Generator cut".to_owned()
            },
            detail: "It will not answer now",
            progress: None,
        },
        AtFixture::Station { powered: false, .. } => PromptView {
            key: String::new(),
            pad: "",
            title: "Station dead".to_owned(),
            detail: "This floor has no power. Find its generator",
            progress: None,
        },
        AtFixture::Station {
            powered: true,
            charge,
        } => PromptView {
            key: String::new(),
            pad: "",
            title: if charge >= MAX_CHARGE {
                "Kinetic tool charged".to_owned()
            } else {
                "Recharging".to_owned()
            },
            detail: "Stand in the cradle to fill the kinetic tool",
            progress: Some(charge as f32 / MAX_CHARGE as f32),
        },
    }
}

/// Whether the local body's floor has power, in a match that has floor power.
pub(in crate::hex_wfc) fn local_floor_powered(runtime: &HexWfcRuntime) -> Option<bool> {
    let ascent = runtime.ascent.as_ref()?;
    Some(
        ascent
            .rules()
            .economy
            .is_powered(runtime.local().cell.level),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_generator_says_what_interact_will_do_and_the_station_its_fill() {
        let settings = Settings::default();
        let cut = prompt_for(
            AtFixture::Generator {
                powered: true,
                operable: true,
            },
            &settings,
        );
        assert_eq!(cut.title, "Cut the floor's power");
        assert!(!cut.key.is_empty(), "an operable generator names its key");
        let restore = prompt_for(
            AtFixture::Generator {
                powered: false,
                operable: true,
            },
            &settings,
        );
        assert_eq!(restore.title, "Restore the floor's power");
        let stuck = prompt_for(
            AtFixture::Generator {
                powered: false,
                operable: false,
            },
            &settings,
        );
        assert!(
            stuck.key.is_empty(),
            "a generator that will not answer names no key"
        );
        let filling = prompt_for(
            AtFixture::Station {
                powered: true,
                charge: MAX_CHARGE / 4,
            },
            &settings,
        );
        assert_eq!(filling.title, "Recharging");
        assert_eq!(filling.progress, Some(0.25));
        assert!(filling.key.is_empty(), "a station is stood at, not pressed");
        let dead = prompt_for(
            AtFixture::Station {
                powered: false,
                charge: 0,
            },
            &settings,
        );
        assert_eq!(dead.title, "Station dead");
        assert_eq!(dead.progress, None);
    }
}
