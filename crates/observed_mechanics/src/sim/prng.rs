//! A SplitMix64 generator, carried by the simulation itself.
//!
//! The lab owns its randomness so a match is reproducible from a `ModeSpec`,
//! a seed and an intent log — and so that no `rand`/`getrandom` dependency
//! reaches the browser build, which is where `tactics_lab` needs an explicit
//! JavaScript entropy backend.

#[derive(Clone, Copy, Debug)]
pub struct Prng {
    state: u64,
}

impl Prng {
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A value in `0..n`, or `None` when `n` is zero.
    pub fn below(&mut self, n: usize) -> Option<usize> {
        (n > 0).then(|| (self.next_u64() % n as u64) as usize)
    }

    /// Deterministic in-place shuffle, Fisher-Yates from the top.
    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = (self.next_u64() % (i as u64 + 1)) as usize;
            items.swap(i, j);
        }
    }
}
