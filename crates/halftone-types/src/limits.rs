//! Safety limits, sized for the 2 GB peak budget on an 8 GB machine (TODO D28/D32).
//! Measured peak is ~10–12 bytes per input pixel (Phase 0 report §A.5) and
//! ~250 bytes per output vertex in the union (§D.3).

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_input_pixels: u64,
    pub max_cells: usize,
    pub max_union_vertices: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_input_pixels: 150_000_000,
            max_cells: 2_000_000,
            max_union_vertices: 6_000_000,
        }
    }
}
