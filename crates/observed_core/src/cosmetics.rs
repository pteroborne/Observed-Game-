//! Stable cosmetic catalog IDs, carried as presentation metadata only.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CosmeticLook {
    pub color: u16,
    pub trail: u16,
    pub badge: u16,
}
impl Default for CosmeticLook {
    fn default() -> Self {
        Self {
            color: 0,
            trail: 4,
            badge: 7,
        }
    }
}
impl CosmeticLook {
    pub const fn is_valid(self) -> bool {
        self.color <= 3 && self.trail >= 4 && self.trail <= 6 && self.badge >= 7 && self.badge <= 9
    }
}
