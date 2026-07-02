//! The loadout screen: a list of cosmetics with a live profile header. Selecting a
//! row equips the cosmetic (via the shared menu activation in [`super::menu`]).

use bevy::prelude::*;
use observed_progression::progression::{Slot, catalog, cosmetic};

use super::{LoadoutHeader, MenuAction, MenuCursor, menu_button};
use crate::GameState;
use crate::flow::Career;
use crate::view::theme::{ACCENT, DIM, TITLE, panel, screen_root, text};

pub(crate) fn setup_loadout(mut commands: Commands, mut cursor: ResMut<MenuCursor>) {
    cursor.0 = 0;
    commands
        .spawn(screen_root(GameState::Loadout))
        .with_children(|root| {
            root.spawn(text("LOADOUT", 40.0, TITLE));
            root.spawn((LoadoutHeader, text("", 17.0, ACCENT)));
            root.spawn(panel()).with_children(|p| {
                let cat = catalog();
                for (index, c) in cat.iter().enumerate() {
                    p.spawn(menu_button(
                        index,
                        MenuAction::Equip(c.id),
                        format!("{}  |  {}", c.name, c.slot.label()),
                    ));
                }
                p.spawn(menu_button(
                    cat.len(),
                    MenuAction::Goto(GameState::MainMenu),
                    "Back",
                ));
            });
            root.spawn(text(
                "Enter equips the selected cosmetic if unlocked | Esc back",
                15.0,
                DIM,
            ));
        });
}

pub(crate) fn loadout_header(
    career: Res<Career>,
    mut header: Query<&mut Text, With<LoadoutHeader>>,
) {
    let profile = &career.profile;
    let equipped = |slot: Slot| {
        profile
            .equipped
            .get(&slot)
            .and_then(|id| cosmetic(*id))
            .map(|c| c.name)
            .unwrap_or("—")
    };
    let Ok(mut header) = header.single_mut() else {
        return;
    };
    **header = format!(
        "Level {} | unlocked {} / {} | equipped  Color {}  Trail {}  Badge {}",
        profile.level(),
        profile.unlocked.len(),
        catalog().len(),
        equipped(Slot::Color),
        equipped(Slot::Trail),
        equipped(Slot::Badge),
    );
}
