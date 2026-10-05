//! Bounded cosmetic metadata; never part of authoritative inputs or digests.
use super::{Cursor, LanCodecError, MAX_SEATS};
use observed_core::cosmetics::CosmeticLook;
use observed_match::hex_wfc::HexMatchConfig;
pub(super) fn encode_look(out: &mut Vec<u8>, look: CosmeticLook) -> Result<(), LanCodecError> {
    if !look.is_valid() {
        return Err(LanCodecError::InvalidValue);
    }
    out.extend([look.color as u8, look.trail as u8, look.badge as u8]);
    Ok(())
}
pub(super) fn decode_look(cursor: &mut Cursor<'_>) -> Result<CosmeticLook, LanCodecError> {
    let look = CosmeticLook {
        color: u16::from(cursor.u8()?),
        trail: u16::from(cursor.u8()?),
        badge: u16::from(cursor.u8()?),
    };
    if !look.is_valid() {
        return Err(LanCodecError::InvalidValue);
    }
    Ok(look)
}
pub(super) fn decode_looks(cursor: &mut Cursor<'_>) -> Result<Vec<CosmeticLook>, LanCodecError> {
    let count = usize::from(cursor.u8()?);
    if count > MAX_SEATS {
        return Err(LanCodecError::InvalidValue);
    }
    (0..count).map(|_| decode_look(cursor)).collect()
}
pub(super) fn validate_looks(
    config: HexMatchConfig,
    ascent: bool,
    looks: &[CosmeticLook],
) -> Result<(), LanCodecError> {
    let seats = usize::from(config.teams) * usize::from(config.members_per_team);
    if seats == 0
        || seats > MAX_SEATS
        || looks.len() != seats
        || looks.iter().any(|l| !l.is_valid())
        || (ascent
            && !(1..=observed_match::ascent::MAX_OBSERVERS_PER_TEAM)
                .contains(&config.members_per_team))
    {
        return Err(LanCodecError::InvalidValue);
    }
    Ok(())
}
