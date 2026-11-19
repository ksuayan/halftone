//! Experiment B: physical rotated Cartesian lattice, whole-cells-only edge policy.
//!
//! Conventions (proposals recorded in the Phase 0 report):
//! - Units: millimetres, x right, y down (SVG orientation).
//! - LPI is the frequency along the lattice axes: pitch = 25.4 / LPI.
//! - Screen origin (phase) = centre of the image rectangle; cell centres sit at
//!   origin + (i + 1/2) u + (j + 1/2) v, where u, v are the rotated basis vectors.
//! - Whole cells only: a cell is kept when a disk of radius `keep_radius` around
//!   its centre lies entirely inside the image rectangle, so no mark is clipped.

use rayon::prelude::*;

#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct Lattice {
    pub pitch: f64,
    pub angle_deg: f64,
    pub origin: (f64, f64),
}

#[derive(Clone, Copy, Debug)]
pub struct Cell {
    pub i: i32,
    pub j: i32,
    pub x: f64,
    pub y: f64,
}

/// Cells plus a dense (i, j) -> cell index map for neighbour queries.
pub struct CellSet {
    pub cells: Vec<Cell>,
    pub i_min: i32,
    pub j_min: i32,
    pub width: usize,
    pub height: usize,
    index: Vec<u32>,
}

impl CellSet {
    pub fn get(&self, i: i32, j: i32) -> Option<usize> {
        let (di, dj) = (i - self.i_min, j - self.j_min);
        if di < 0 || dj < 0 || di as usize >= self.width || dj as usize >= self.height {
            return None;
        }
        let v = self.index[dj as usize * self.width + di as usize];
        (v != u32::MAX).then_some(v as usize)
    }
}

impl Lattice {
    pub fn from_lpi(lpi: f64, angle_deg: f64, rect: Rect) -> Self {
        Self {
            pitch: 25.4 / lpi,
            angle_deg,
            origin: ((rect.x0 + rect.x1) / 2.0, (rect.y0 + rect.y1) / 2.0),
        }
    }

    fn basis(&self) -> ((f64, f64), (f64, f64)) {
        let (s, c) = self.angle_deg.to_radians().sin_cos();
        ((c, s), (-s, c))
    }

    pub fn center(&self, i: i32, j: i32) -> (f64, f64) {
        let ((ux, uy), (vx, vy)) = self.basis();
        let (a, b) = ((i as f64 + 0.5) * self.pitch, (j as f64 + 0.5) * self.pitch);
        (self.origin.0 + a * ux + b * vx, self.origin.1 + a * uy + b * vy)
    }

    /// Inverse transform: physical point -> continuous lattice coordinates.
    pub fn to_lattice(&self, x: f64, y: f64) -> (f64, f64) {
        let ((ux, uy), (vx, vy)) = self.basis();
        let (dx, dy) = (x - self.origin.0, y - self.origin.1);
        ((dx * ux + dy * uy) / self.pitch - 0.5, (dx * vx + dy * vy) / self.pitch - 0.5)
    }

    pub fn whole_cells(&self, rect: Rect, keep_radius: f64) -> CellSet {
        let corners = [(rect.x0, rect.y0), (rect.x1, rect.y0), (rect.x0, rect.y1), (rect.x1, rect.y1)];
        let (mut a0, mut a1, mut b0, mut b1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for (x, y) in corners {
            let (a, b) = self.to_lattice(x, y);
            a0 = a0.min(a);
            a1 = a1.max(a);
            b0 = b0.min(b);
            b1 = b1.max(b);
        }
        let (i_min, i_max) = (a0.floor() as i32 - 1, a1.ceil() as i32 + 1);
        let (j_min, j_max) = (b0.floor() as i32 - 1, b1.ceil() as i32 + 1);
        let inset = Rect {
            x0: rect.x0 + keep_radius,
            y0: rect.y0 + keep_radius,
            x1: rect.x1 - keep_radius,
            y1: rect.y1 - keep_radius,
        };
        // Rows in parallel; indexed collect keeps (j, i) order -> deterministic.
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
        CellSet { cells, i_min, j_min, width, height, index }
    }
}
