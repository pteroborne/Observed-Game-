//! Seat commands on the wire: what an Architect or a body asks of the Ascent rules, carried
//! beside each body command so every peer applies it on the same tick.
//!
//! A body's movement is a [`WireHexCommand`]; its seat's say in the rules - a card played,
//! a requisition, a Rogue's directive or sensor, an ask for help, an answer to one - rides along as a
//! [`WireSeatCommand`], nothing on most ticks. The server puts each seat's into the frame,
//! and every peer maps it to a rules seat the same way (`observed_match::ascent::facility`
//! `seat_for`).

use observed_core::PlayerId;
use observed_facility::hex_wfc::HexCoord;
use observed_match::ascent::session::{RequestKind, SeatCommand};
use observed_match::ascent::sim::{ArchitectCommand, CardId};

use super::{Cursor, LanCodecError, put_u16, put_u32, put_u64};

/// One seat's command this tick, as the wire carries it.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WireSeatCommand {
    #[default]
    None,
    Play {
        card: u32,
        target: HexCoord,
        rotation: u8,
    },
    Requisition,
    Request {
        kind: RequestKind,
        target: HexCoord,
    },
    Acknowledge {
        author: PlayerId,
        created_at: u64,
    },
    Direct {
        target: HexCoord,
    },
    Sense {
        target: HexCoord,
    },
}

impl WireSeatCommand {
    /// The most bytes one takes on the wire: a tag and an acknowledgement or a play.
    pub const MAX_BYTES: usize = 11;

    /// The wire form of `command`, or nothing for what does not travel (an Observer's
    /// own rules command: a body moves by its body command).
    #[must_use]
    pub fn from_seat(command: SeatCommand) -> Self {
        match command {
            SeatCommand::Architect(ArchitectCommand::Play {
                card,
                target,
                rotation,
            }) => Self::Play {
                card: card.0,
                target,
                rotation,
            },
            SeatCommand::Architect(ArchitectCommand::Requisition) => Self::Requisition,
            SeatCommand::Architect(ArchitectCommand::Direct { target }) => Self::Direct { target },
            SeatCommand::Architect(ArchitectCommand::Sense { target }) => Self::Sense { target },
            SeatCommand::Request { kind, target } => Self::Request { kind, target },
            SeatCommand::Acknowledge { author, created_at } => {
                Self::Acknowledge { author, created_at }
            }
            SeatCommand::None | SeatCommand::Observer(_) => Self::None,
        }
    }

    /// The rules command this carries, or `None` for nothing.
    #[must_use]
    pub fn to_seat(self) -> Option<SeatCommand> {
        Some(match self {
            Self::None => return None,
            Self::Play {
                card,
                target,
                rotation,
            } => SeatCommand::Architect(ArchitectCommand::Play {
                card: CardId(card),
                target,
                rotation,
            }),
            Self::Requisition => SeatCommand::Architect(ArchitectCommand::Requisition),
            Self::Direct { target } => SeatCommand::Architect(ArchitectCommand::Direct { target }),
            Self::Sense { target } => SeatCommand::Architect(ArchitectCommand::Sense { target }),
            Self::Request { kind, target } => SeatCommand::Request { kind, target },
            Self::Acknowledge { author, created_at } => {
                SeatCommand::Acknowledge { author, created_at }
            }
        })
    }

    pub(super) fn encode(self, out: &mut Vec<u8>) {
        let cell = |out: &mut Vec<u8>, cell: HexCoord| {
            put_u16(out, cell.q);
            put_u16(out, cell.r);
            out.push(cell.level);
        };
        match self {
            Self::None => out.push(0),
            Self::Play {
                card,
                target,
                rotation,
            } => {
                out.push(1);
                put_u32(out, card);
                cell(out, target);
                out.push(rotation);
            }
            Self::Requisition => out.push(2),
            Self::Request { kind, target } => {
                out.push(3);
                out.push(kind_code(kind));
                cell(out, target);
            }
            Self::Acknowledge { author, created_at } => {
                out.push(4);
                put_u16(out, author.0);
                put_u64(out, created_at);
            }
            Self::Direct { target } => {
                out.push(5);
                cell(out, target);
            }
            Self::Sense { target } => {
                out.push(6);
                cell(out, target);
            }
        }
    }

    pub(super) fn decode(cursor: &mut Cursor<'_>) -> Result<Self, LanCodecError> {
        let cell = |cursor: &mut Cursor<'_>| -> Result<HexCoord, LanCodecError> {
            Ok(HexCoord {
                q: cursor.u16()?,
                r: cursor.u16()?,
                level: cursor.u8()?,
            })
        };
        Ok(match cursor.u8()? {
            0 => Self::None,
            1 => Self::Play {
                card: cursor.u32()?,
                target: cell(cursor)?,
                rotation: cursor.u8()?,
            },
            2 => Self::Requisition,
            3 => Self::Request {
                kind: kind_from(cursor.u8()?)?,
                target: cell(cursor)?,
            },
            4 => Self::Acknowledge {
                author: PlayerId(cursor.u16()?),
                created_at: cursor.u64()?,
            },
            5 => Self::Direct {
                target: cell(cursor)?,
            },
            6 => Self::Sense {
                target: cell(cursor)?,
            },
            _ => return Err(LanCodecError::InvalidValue),
        })
    }
}

const KINDS: [RequestKind; 6] = [
    RequestKind::Route,
    RequestKind::Power,
    RequestKind::Recharge,
    RequestKind::Rescue,
    RequestKind::HoldObservation,
    RequestKind::ReleaseObservation,
];

fn kind_code(kind: RequestKind) -> u8 {
    KINDS
        .iter()
        .position(|&known| known == kind)
        .and_then(|index| u8::try_from(index).ok())
        .expect("every request kind has a code")
}

fn kind_from(code: u8) -> Result<RequestKind, LanCodecError> {
    KINDS
        .get(usize::from(code))
        .copied()
        .ok_or(LanCodecError::InvalidValue)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_seat_command_crosses_the_wire_and_back_within_its_budget() {
        let cell = HexCoord {
            q: 513,
            r: 7,
            level: 9,
        };
        let mut commands = vec![
            WireSeatCommand::None,
            WireSeatCommand::Play {
                card: 70_000,
                target: cell,
                rotation: 5,
            },
            WireSeatCommand::Requisition,
            WireSeatCommand::Direct { target: cell },
            WireSeatCommand::Sense { target: cell },
            WireSeatCommand::Acknowledge {
                author: PlayerId(3),
                created_at: u64::MAX - 1,
            },
        ];
        commands.extend(KINDS.map(|kind| WireSeatCommand::Request { kind, target: cell }));
        for command in commands {
            let mut out = Vec::new();
            command.encode(&mut out);
            assert!(out.len() <= WireSeatCommand::MAX_BYTES, "{command:?}");
            let mut cursor = Cursor::new(&out);
            assert_eq!(WireSeatCommand::decode(&mut cursor), Ok(command));
            assert_eq!(cursor.remaining(), 0);
            assert_eq!(
                command
                    .to_seat()
                    .map_or(WireSeatCommand::None, WireSeatCommand::from_seat),
                command
            );
        }
    }
}
