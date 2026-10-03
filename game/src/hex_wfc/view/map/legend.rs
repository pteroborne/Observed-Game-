//! Plain-language map summary, with the full sketch vocabulary on demand.
use super::build::MapCensus;
use crate::hex_wfc::{hud::guidance, sim::HexWfcRuntime};
use crate::settings::{Settings, key_name};

pub(super) fn summary(
    census: &MapCensus,
    runtime: &HexWfcRuntime,
    settings: &Settings,
    heading: Option<(&'static str, f32)>,
    expanded: bool,
) -> String {
    let objective = guidance::objective(runtime);
    let position = if runtime.local().place == observed_match::hex_wfc::HexBodyPlace::Prison {
        "You are in prison. This sketch shows the facility, not the prison maze.".to_owned()
    } else if runtime.local().escaped {
        "Your Observer has escaped.".to_owned()
    } else if runtime.local().place == observed_match::hex_wfc::HexBodyPlace::Void {
        "Your Observer is outside the facility.".to_owned()
    } else {
        format!(
            "You: floor {} / cyan arrow. Green: discovered exit.",
            u16::from(runtime.local().cell.level) + 1
        )
    };
    let mut text = format!(
        "TEAM MAP / FLOOR {}\n{}\n{position}\nKnown floors: {}\nPgUp PgDn / D-pad up down  floor\nH / X  {}    {} / RB or Esc / B  close",
        u16::from(runtime.map_level) + 1,
        objective.goal,
        known_floors(census, runtime.map_level),
        if expanded {
            "less detail"
        } else {
            "reading guide"
        },
        key_name(settings.bindings.tac_map),
    );
    if expanded {
        text.push_str("\n\n");
        text.push_str(&details(census, heading, runtime.ascent.is_some()));
    }
    text
}

fn known_floors(census: &MapCensus, focus: u8) -> String {
    if census.floors.is_empty() {
        return "none yet".to_owned();
    }
    census
        .floors
        .iter()
        .map(|&floor| {
            let label = u16::from(floor) + 1;
            if floor == focus {
                format!("[{label}]")
            } else {
                label.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) fn details(
    census: &MapCensus,
    heading: Option<(&'static str, f32)>,
    ascent: bool,
) -> String {
    let rooms = if census.rooms.is_empty() {
        "none yet".to_owned()
    } else {
        census.rooms.iter().cloned().collect::<Vec<_>>().join(", ")
    };
    let heading = heading.map_or_else(
        || "facing: unavailable".to_owned(),
        |(label, deg)| format!("facing: {label} ({deg:.0} deg)"),
    );
    let stability = if ascent {
        "Rooms may change. Climbs and fixed structure remain. Occupancy does not stop retraction."
    } else {
        "Rooms and vertical links remain. Corridors may rewire unless held."
    };
    let cap = if ascent {
        "capped = known anchor"
    } else {
        "capped = anchor / teammate"
    };
    format!(
        "Only your team's discoveries are drawn.\n{} traversed | {} glimpsed | {} stale\nStale cells may have changed since you saw them.\nRooms: {rooms}\n{} lateral | {} vertical links seen from both sides\n{} may change | {} permanent | {} held\n{stability}\ncolour = district | width = room/hallway\nheight = archetype | {cap}\ncyan = you & facing | green = exit\npurple = device/held | amber = room\norientation: N (up-left) E (up-right) S (down-right) W (down-left)\n{heading}",
        census.traversed,
        census.glimpsed,
        census.stale,
        census.lateral_links,
        census.vertical_links,
        census.mutable,
        census.permanent,
        census.held,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floor_labels_match_the_hud_and_include_only_discovered_floors() {
        let census = MapCensus {
            floors: [0, 2, 7].into_iter().collect(),
            ..Default::default()
        };
        assert_eq!(known_floors(&census, 2), "1 [3] 8");
        assert_eq!(known_floors(&MapCensus::default(), 0), "none yet");
    }

    #[test]
    fn ascent_guide_does_not_promise_that_rooms_or_occupancy_are_safe() {
        let text = details(&MapCensus::default(), None, true);
        assert!(text.contains("Rooms may change"));
        assert!(text.contains("Occupancy does not stop retraction"));
        assert!(!text.contains("Rooms and vertical links remain"));
        assert!(text.is_ascii());
    }
}
