//! Seeded scenarios for the GFS teaching model.
//!
//! This is not a multi-process network fault harness; production M5/M6 tests
//! remain open. Every action in this model is reproducible from its seed.
#![forbid(unsafe_code)]

pub struct Seeded {
    state: u64,
}
impl Seeded {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }
}
