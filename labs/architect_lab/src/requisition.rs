//! Emergency requisition mechanics: refills the Architect hand to exactly five,
//! releasing exactly one major Guardian on the floor active Observers occupy,
//! visible to every faction, leaving the placement cooldown untouched.

/// Deterministic PRNG following SplitMix discipline.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequisitionPrng(pub u64);

impl RequisitionPrng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        value ^ (value >> 31)
    }

    #[must_use]
    pub fn below(&mut self, limit: usize) -> usize {
        if limit == 0 {
            0
        } else {
            (self.next() % limit as u64) as usize
        }
    }
}

/// Simulation-level tracking for emergency requisitions.
#[derive(Clone, Debug, PartialEq)]
pub struct RequisitionState {
    pub count: u32,
    pub next_guardian_id: u32,
    pub rng: RequisitionPrng,
}

impl RequisitionState {
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self {
            count: 0,
            next_guardian_id: 1, // GuardianId(0) is reserved for the initial guardian
            rng: RequisitionPrng(seed.wrapping_add(0x517C_C1B7_2722_0A95)),
        }
    }
}
