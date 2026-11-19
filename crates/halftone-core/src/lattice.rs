//! Rotated Cartesian lattice (decision D3, Phase 0 report §B).
//!
//! - Units: millimetres, x right, y down.
//! - LPI is the frequency along the lattice axes: pitch = 25.4 / LPI.
//! - Phase origin = centre of the image rectangle; cell (i, j) is centred at
//!   origin + (i + 1/2) u + (j + 1/2) v, with u, v the rotated unit axes × pitch.
//! - Whole cells only (default edge policy): a cell is kept when a disk of
//!   radius `keep_radius` around its centre lies inside the rectangle.

use rayon::prelude::*;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

impl Rect {
    pub fn width(&self) -> f64 {
        self.x1 - self.x0
    }
    pub fn height(&self) -> f64 {
        self.y1 - self.y0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Lattice {
    pub pitch: f64,
    pub angle_deg: f64,
    pub origin: (f64, f64),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cell {
    pub i: i32,
    pub j: i32,
    pub x: f64,
    pub y: f64,
}

/// Kept cells plus a dense (i, j) -> index map for neighbour queries.
pub struct CellSet {
    pub cells: Vec<Cell>,
    i_min: i32,
    j_min: i32,
    width: usize,
    height: usize,
    index: Vec<u32>,
}

impl CellSet {
    pub fn index_of(&self, i: i32, j: i32) -> Option<usize> {
        let (di, dj) = (i - self.i_min, j - self.j_min);
        if di < 0 || dj < 0 || di as usize >= self.width || dj as usize >= self.height {
            return None;
        }
        let v = self.index[dj as usize * self.width + di as usize];
        (v != u32::MAX).then_some(v as usize)
    }
}

impl Lattice {
    pub fn new(pitch: f64, angle_deg: f64, rect: &Rect) -> Self {
        Self {
            pitch,
            angle_deg,
            origin: ((rect.x0 + rect.x1) / 2.0, (rect.y0 + rect.y1) / 2.0),
        }
    }

    fn axes(&self) -> ((f64, f64), (f64, f64)) {
        let (s, c) = self.angle_deg.to_radians().sin_cos();
        ((c, s), (-s, c))
    }

    pub fn center(&self, i: i32, j: i32) -> (f64, f64) {
        let ((ux, uy), (vx, vy)) = self.axes();
        let (a, b) = ((i as f64 + 0.5) * self.pitch, (j as f64 + 0.5) * self.pitch);
        (self.origin.0 + a * ux + b * vx, self.origin.1 + a * uy + b * vy)
    }

    /// Physical point -> continuous lattice coordinates (cell (i, j) centre -> (i, j)).
    pub fn to_lattice(&self, x: f64, y: f64) -> (f64, f64) {
        let ((ux, uy), (vx, vy)) = self.axes();
        let (dx, dy) = (x - self.origin.0, y - self.origin.1);
        (
            (dx * ux + dy * uy) / self.pitch - 0.5,
            (dx * vx + dy * vy) / self.pitch - 0.5,
        )
    }

    /// Upper bound on cells covering `rect` (for limit checks before allocating).
    pub fn estimate_cells(&self, rect: &Rect) -> f64 {
        rect.width() * rect.height() / (self.pitch * self.pitch)
    }

    pub fn whole_cells(&self, rect: &Rect, keep_radius: f64) -> CellSet {
        let corners = [
            (rect.x0, rect.y0),
            (rect.x1, rect.y0),
            (rect.x0, rect.y1),
            (rect.x1, rect.y1),
        ];
        let (mut a0, mut a1, mut b0, mut b1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for (x, y) in corners {
            let (a, b) = self.to_lattice(x, y);
            (a0, a1, b0, b1) = (a0.min(a), a1.max(a), b0.min(b), b1.max(b));
        }
        let (i_min, i_max) = (a0.floor() as i32 - 1, a1.ceil() as i32 + 1);
        let (j_min, j_max) = (b0.floor() as i32 - 1, b1.ceil() as i32 + 1);
        let inset = Rect {
            x0: rect.x0 + keep_radius,
            y0: rect.y0 + keep_radius,
            x1: rect.x1 - keep_radius,
            y1: rect.y1 - keep_radius,
        };
        // Rows in parallel; ordered collect keeps (j, i) order -> deterministic.
        let rows: Vec<Vec<Cell>> = (j_min..=j_max)
            .into_par_iter()
            .map(|j| {
                (i_min..=i_max)
                    .filter_map(|i| {
                        let (x, y) = self.center(i, j);
                        let inside = x >= inset.x0 && x <= inset.x1 && y >= inset.y0 && y <= inset.y1;
                        inside.then_some(Cell { i, j, x, y })
                    })
                    .collect()
            })
            .collect();
        let cells: Vec<Cell> = rows.into_iter().flatten().collect();
        let width = (i_max - i_min + 1) as usize;
        let height = (j_max - j_min + 1) as usize;
        let mut index = vec![u32::MAX; width * height];
        for (k, c) in cells.iter().enumerate() {
            index[(c.j - j_min) as usize * width + (c.i - i_min) as usize] = k as u32;
        }
        CellSet {
            cells,
            i_min,
            j_min,
            width,
            height,
            index,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_cells_stay_inside_and_round_trip() {
        let rect = Rect {
            x0: 10.0,
            y0: 20.0,
            x1: 110.0,
            y1: 170.0,
        };
        for angle in [0.0, 15.0, 22.5, 45.0, 89.9] {
            let lat = Lattice::new(1.0, angle, &rect);
            let keep = 0.75;
            let set = lat.whole_cells(&rect, keep);
            assert!(!set.cells.is_empty());
            for c in &set.cells {
                assert!(c.x - keep >= rect.x0 - 1e-9 && c.x + keep <= rect.x1 + 1e-9);
                assert!(c.y - keep >= rect.y0 - 1e-9 && c.y + keep <= rect.y1 + 1e-9);
                let (a, b) = lat.to_lattice(c.x, c.y);
                assert!((a - c.i as f64).abs() < 1e-9 && (b - c.j as f64).abs() < 1e-9);
                assert_eq!(set.cells[set.index_of(c.i, c.j).unwrap()], *c);
            }
            // Roughly area / pitch^2 minus the excluded border.
            let expected = (rect.width() - 2.0 * keep) * (rect.height() - 2.0 * keep);
            assert!((set.cells.len() as f64 - expected).abs() < 0.05 * expected, "{angle}");
        }
    }

    #[test]
    fn deterministic_order() {
        let rect = Rect {
            x0: 0.0,
            y0: 0.0,
            x1: 50.0,
            y1: 40.0,
        };
        let lat = Lattice::new(0.7, 22.5, &rect);
        assert_eq!(lat.whole_cells(&rect, 0.5).cells, lat.whole_cells(&rect, 0.5).cells);
    }
}
