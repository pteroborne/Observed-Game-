//! Play a match from the terminal.
//!
//! There is no saved state. The whole order log is replayed from the seed every
//! invocation, which is only possible because a match is reproducible from a
//! `ModeSpec` plus its intent log — the property `sim::tests` pins. So a turn is
//! "append to the log and run it again", and the board that comes back is
//! exactly the board that would have come back had it been played in one go.
//!
//! ```text
//! cargo run -p mechanic_lab --example play -- plant
//! cargo run -p mechanic_lab --example play -- plant "0:E 1:E 2:NE"
//! cargo run -p mechanic_lab --example play -- plant "0:E 1:E 2:NE / 0:H@NE 1:P 2:W"
//! ```
//!
//! Orders are `id:ACTION[@FACE]`, space separated, turns divided by `/`.
//! ACTION is a face (`E SE SW W NW NE`) to step, `H` to hold, `P` to plant.
//! `@FACE` sets the facing separately, which is how you arrive looking
//! somewhere other than the way you walked.

use mechanic_lab::sim::board::Edge;
use mechanic_lab::sim::state::{Action, ChangeSource, Intent, MatchState, PawnId};
use mechanic_lab::sim::tiles::{TilePlay, TileShape};
use mechanic_lab::sim::{bot, step::step};
use mechanic_lab::spec::{ModeSpec, Rules, deal};
use observed_hex::coords::HexCoord;
use observed_hex::faces::HexFace;
use observed_hex::ports::PortClass;

fn face_of(token: &str) -> Option<HexFace> {
    Some(match token.to_ascii_uppercase().as_str() {
        "E" => HexFace::East,
        "SE" => HexFace::SouthEast,
        "SW" => HexFace::SouthWest,
        "W" => HexFace::West,
        "NW" => HexFace::NorthWest,
        "NE" => HexFace::NorthEast,
        _ => return None,
    })
}

fn short(face: HexFace) -> &'static str {
    match face {
        HexFace::East => "E",
        HexFace::SouthEast => "SE",
        HexFace::SouthWest => "SW",
        HexFace::West => "W",
        HexFace::NorthWest => "NW",
        HexFace::NorthEast => "NE",
        _ => "?",
    }
}

fn shape_of(token: &str) -> Option<TileShape> {
    TileShape::ALL
        .into_iter()
        .find(|shape| shape.label().replace(' ', "") == token.to_ascii_lowercase())
}

/// Parse one turn's worth of orders for the human team.
fn parse_turn(text: &str, state: &MatchState) -> (Vec<Intent>, Vec<TilePlay>) {
    let mut out = Vec::new();
    let mut plays = Vec::new();
    for token in text.split_whitespace() {
        // An architect play: `A:q,r,shape,rotation`.
        if let Some(rest) = token.strip_prefix("A:") {
            let parts: Vec<&str> = rest.split(',').collect();
            let parsed = (parts.len() == 4)
                .then(|| {
                    Some(TilePlay {
                        cell: HexCoord {
                            q: parts[0].parse().ok()?,
                            r: parts[1].parse().ok()?,
                            level: 0,
                        },
                        shape: shape_of(parts[2])?,
                        rotation: parts[3].parse().ok()?,
                    })
                })
                .flatten();
            match parsed {
                Some(play) => plays.push(play),
                None => eprintln!("skipping `{token}`: expected A:q,r,shape,rotation"),
            }
            continue;
        }
        let Some((id, rest)) = token.split_once(':') else {
            eprintln!("skipping `{token}`: expected id:ACTION");
            continue;
        };
        let Ok(id) = id.parse::<u8>() else {
            eprintln!("skipping `{token}`: bad pawn id");
            continue;
        };
        let (action_text, facing_text) = match rest.split_once('@') {
            Some((a, f)) => (a, Some(f)),
            None => (rest, None),
        };
        let pawn = PawnId(id);
        let current = state.pawn(pawn).facing;
        let action = match action_text.to_ascii_uppercase().as_str() {
            "H" => Action::Hold,
            "P" => Action::Plant,
            other => match face_of(other) {
                Some(face) => Action::Step(face),
                None => {
                    eprintln!("skipping `{token}`: unknown action");
                    continue;
                }
            },
        };
        let facing = facing_text
            .and_then(face_of)
            .or(match action {
                Action::Step(face) => Some(face),
                _ => None,
            })
            .unwrap_or(current);
        out.push(Intent {
            pawn,
            facing,
            action,
        });
    }
    (out, plays)
}

/// One glyph for whatever is standing here.
fn glyph(state: &MatchState, cell: HexCoord) -> char {
    if let Some(guardian) = state.guardians.iter().find(|g| g.at == cell) {
        let _ = guardian;
        return 'G';
    }
    let here = state.occupants(cell);
    if let Some(&first) = here.first() {
        let pawn = state.pawn(first);
        return if pawn.team.0 == 0 {
            char::from_digit(u32::from(first.0), 10).unwrap_or('P')
        } else {
            // p,q,r... deliberately: `f` would collide with a planted flag and
            // `b` with a base, and a glyph that means two things is worse than
            // an ugly one.
            char::from(b'p' + first.0 % 10)
        };
    }
    if state.pawns.iter().any(|p| p.jailed && p.at == cell) {
        return 'j';
    }
    if let Some(flag) = state.flags.iter().find(|f| f.at == cell) {
        return if flag.planted_by.is_some() { 'f' } else { 'F' };
    }
    if state.prisons.contains(&cell) {
        return 'J';
    }
    if state.spawns.contains(&cell) {
        return 'B';
    }
    '.'
}

/// The character for a boundary: a wall, a doorway, or what it is about to
/// become.
fn boundary(state: &MatchState, edge: Edge, wall: char) -> char {
    let canonical = edge.canonical(state.board.size());
    if let Some(change) = state.telegraph.iter().find(|c| c.edge == canonical) {
        // A played tile and a random rewire must not look the same, or the
        // experiment cannot be run: `#`/`=` is somebody's decision, `x`/`o` is
        // the facility decohering on its own.
        return match (change.source, change.to) {
            (ChangeSource::Architect(_), PortClass::Sealed) => '#',
            (ChangeSource::Architect(_), _) => '=',
            (_, PortClass::Sealed) => 'x',
            (_, _) => 'o',
        };
    }
    if state.board.port(edge) == PortClass::Door {
        ' '
    } else {
        wall
    }
}

/// Draw the hexagon. Rows are staggered; `|` is an east-west wall, `/` and `\`
/// are the diagonal ones, and `X`/`o` mark a boundary about to close or open.
fn draw(state: &MatchState) {
    let span = state.board.radius() * 2 + 1;
    let width = (span as usize + 2) * 4 + 8;

    for r in 0..span {
        let mut cells = vec![' '; width];
        let mut below = vec![' '; width];
        for q in 0..span {
            let cell = HexCoord { q, r, level: 0 };
            if !state.board.on_board(cell) {
                continue;
            }
            let col = 4 * q as usize + 2 * r as usize;
            cells[col] = '(';
            cells[col + 1] = glyph(state, cell);
            cells[col + 2] = ')';
            cells[col + 3] = boundary(
                state,
                Edge {
                    cell,
                    face: HexFace::East,
                },
                '|',
            );
            below[col] = boundary(
                state,
                Edge {
                    cell,
                    face: HexFace::SouthWest,
                },
                '/',
            );
            below[col + 2] = boundary(
                state,
                Edge {
                    cell,
                    face: HexFace::SouthEast,
                },
                '\\',
            );
        }
        println!("{}", String::from_iter(&cells).trim_end());
        let line = String::from_iter(&below);
        if !line.trim().is_empty() {
            println!("{}", line.trim_end());
        }
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let which = args.next().unwrap_or_else(|| "plant".into());
    let log = args.next().unwrap_or_default();
    // In a duel nobody is driven: every order in the log is somebody's choice,
    // and an unmentioned pawn holds. That is what lets two players — or two
    // agents — share one deterministic replay.
    let duel = args.any(|arg| arg == "--duel");

    let spec = ModeSpec::presets()
        .into_iter()
        .find(|s| s.name.to_ascii_lowercase().replace([' ', ':'], "") == which.to_ascii_lowercase())
        .unwrap_or_else(|| match which.as_str() {
            "base" => ModeSpec::base(),
            _ => ModeSpec::plant(),
        });

    let rules = Rules::from_spec(&spec);
    let mut state = deal(&spec);

    for turn in log.split('/').filter(|t| !t.trim().is_empty()) {
        if state.outcome.is_some() {
            break;
        }
        let (mut intents, plays) = parse_turn(turn, &state);
        state.architect_queue = plays;
        // Anyone unmentioned holds; other teams are driven unless this is a duel.
        let mine: Vec<PawnId> = state
            .free_pawns()
            .filter(|p| duel || p.team.0 == 0)
            .map(|p| p.id)
            .collect();
        for pawn in mine {
            if !intents.iter().any(|i| i.pawn == pawn) {
                let facing = state.pawn(pawn).facing;
                intents.push(Intent {
                    pawn,
                    facing,
                    action: Action::Hold,
                });
            }
        }
        if !duel {
            for team in state.teams().into_iter().filter(|t| t.0 != 0) {
                intents.extend(bot::team_intents(&state, team));
            }
        }
        intents.sort_by_key(|i| i.pawn);
        step(&mut state, &rules, &intents);
    }

    println!("mode   : {}   [{}]", spec.name, rules.summary());
    println!(
        "turn   : {}/{}   outcome {:?}",
        state.turn, spec.turn_limit, state.outcome
    );
    let flags: Vec<String> = state
        .flags
        .iter()
        .map(|f| {
            format!(
                "{:?}{}",
                (f.at.q, f.at.r),
                if f.planted_by.is_some() { "*" } else { "" }
            )
        })
        .collect();
    println!("flags  : {}", flags.join("  "));
    println!();
    draw(&state);
    println!();

    for pawn in &state.pawns {
        if !duel && pawn.team.0 != 0 {
            continue;
        }
        let moves: Vec<&str> = state
            .board
            .open_neighbours(pawn.at)
            .map(|(face, _)| short(face))
            .collect();
        println!(
            "pawn {} (team {}) at {:?} facing {:<2} {} | exits: {}",
            pawn.id.0,
            pawn.team.0,
            (pawn.at.q, pawn.at.r),
            short(pawn.facing),
            if pawn.jailed { "HELD " } else { "     " },
            moves.join(" "),
        );
    }
    for (index, guardian) in state.guardians.iter().enumerate() {
        println!("guardian {index} at {:?}", (guardian.at.q, guardian.at.r));
    }

    if let Some(hand) = state.hand_of(mechanic_lab::sim::state::TeamId(0))
        && !hand.cards.is_empty()
    {
        let cards: Vec<&str> = hand.cards.iter().map(|card| card.label()).collect();
        println!();
        println!(
            "hand   : {}   ({} play/turn, {} owed, {} left in deck)",
            cards.join(", "),
            hand.plays_per_turn,
            hand.owed,
            hand.deck.len()
        );
    }
    for (play, refusal) in &state.refusals {
        println!(
            "REFUSED: {} at {:?} rot {} - {}",
            play.shape.label(),
            (play.cell.q, play.cell.r),
            play.rotation,
            refusal.label()
        );
    }
    if !state.report.tiles_played.is_empty() {
        let played: Vec<String> = state
            .report
            .tiles_played
            .iter()
            .map(|play| format!("{} at {:?}", play.shape.label(), (play.cell.q, play.cell.r)))
            .collect();
        println!("played : {}", played.join("   "));
    }
    println!();
    println!(
        "key: 0-9=team0 pawns  p,q,r..=team1 pawns  G=guardian  F=flag f=planted  J=prison  B=base  j=held"
    );
    println!("     | / \\ = wall   # = architect walls up  = architect opens   x/o = rogue churn");
}
