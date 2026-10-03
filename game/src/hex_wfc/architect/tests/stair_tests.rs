//! A stair played at the desk: the climb composition it builds, and what the Architect
//! believes stands on both floors.

use super::*;

#[test]
fn a_stair_played_at_the_desk_is_built_and_believed_on_both_floors() {
    use observed_facility::hex_wfc::HexArchetype;
    use observed_match::ascent::sim::CardKind;

    let mut runtime = runtime();
    let mut desk = desk();
    let stair = |runtime: &HexWfcRuntime, desk: &ArchitectDesk| {
        let ascent = runtime.ascent.as_ref()?;
        let known = &ascent.rules().team_knowledge.get(&desk.team)?.cells;
        let hand = &ascent.session().hands.get(&desk.team)?.deck.hand;
        hand.iter()
            .filter(|card| card.kind == CardKind::Stair)
            .find_map(|card| {
                known.keys().find_map(|&target| {
                    (0..6).find_map(|rotation| {
                        let play = ArchitectCommand::Play {
                            card: card.id,
                            target,
                            rotation,
                        };
                        ascent
                            .session()
                            .architect_refusal(desk.seat, play)
                            .is_none()
                            .then_some(play)
                    })
                })
            })
    };
    let mut play = None;
    for tick_count in 0..6_000 {
        play = stair(&runtime, &desk);
        if play.is_some() {
            break;
        }
        // A hand without a stair draws again now and then.
        if tick_count % 600 == 599 {
            desk.pending = Some(ArchitectCommand::Requisition);
        }
        tick(&mut runtime, &mut desk);
    }
    let play = play.expect("a stair becomes legal as the team maps the ground floor");
    let ArchitectCommand::Play { target, .. } = play else {
        unreachable!()
    };
    desk.pending = Some(play);
    tick(&mut runtime, &mut desk);
    assert_eq!(desk.last_refusal, None);
    let facility = &runtime.match_state.facility;
    let physical = &facility.placements;
    assert!(matches!(
        physical[&target].archetype,
        HexArchetype::Climb {
            part: observed_facility::hex_wfc::ClimbPart::Foot,
            ..
        }
    ));
    let cells = observed_facility::hex_wfc::composition_cells(
        facility.config.grid(),
        target,
        physical[&target].archetype,
    )
    .expect("a whole climb composition");
    assert_eq!(
        cells[3].level,
        target.level + 1,
        "its landing is a floor up"
    );
    // The Architect knows all four cells stand, the landing on a floor nobody has
    // reached: from what it built, or from what the team has seen of it since (a body may
    // be looking at the foot from beyond its ward), as the board reads it.
    let rules = runtime.ascent.as_ref().expect("Ascent rules").rules();
    let knowledge = desk.knowledge(rules).expect("the team's knowledge");
    for cell in cells {
        assert_eq!(
            desk.believed(cell, knowledge.cells.get(&cell))
                .map(|p| p.archetype),
            Some(physical[&cell].archetype),
            "{cell:?}"
        );
    }
    // And the board's ghost and the hand's miniature can draw one.
    let pieces = super::super::building::built_by(&runtime.match_state, CardKind::Stair, target, 0)
        .expect("the corpus builds a stair");
    assert!(pieces.iter().any(|piece| piece.source_cell == target));
}
