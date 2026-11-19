# Architecture

Seven crates (decision D21). Dependencies point one way; libraries never depend on the CLI.

```text
halftone-cli ──► halftone ──► halftone-image ──┐
                     │                         │
                     ├──► halftone-core ───────┤
                     ├──► halftone-geometry ───┼──► halftone-types ──► geo-types
                     └──► halftone-svg ────────┘
```

| Crate | Responsibility | Key dependencies |
| --- | --- | --- |
| `halftone-types` | Units (mm config, µm geometry), paper sizes, the `Config` schema and its validation, geometry conventions (`geo-types` `MultiPolygon<i32>`, D33), limits | `geo-types`, `serde`, `schemars` |
| `halftone-image` | Decode JPEG/PNG/TIFF, ICC → linear luminance, alpha over white, EXIF orientation, box-filter downscale | `image`, `moxcms`, `fast_image_resize`, `rayon` |
| `halftone-core` | Tone curve, periodic coverage ↔ diameter, rotated lattice (whole cells), mark placement. Reads luminance through a closure, so it is image-agnostic | `rayon` |
| `halftone-geometry` | Area-preserving circle polygons; union of overlapping marks only (`i_overlay`); `geo-types` output | `i_overlay`, `geo-types`, `rayon` |
| `halftone-svg` | SVG writer (µm `viewBox`, one compound path, registration targets), output validator, PNG preview | `roxmltree`, `tiny-skia` |
| `halftone` | Pipeline: `analyze` (header-only probe → recommendations → `Config`) and `generate` (`Config` → validated SVG); profiles; page layout; config I/O | all of the above, `blake3` (`pure`) |
| `halftone-cli` | `halftone` binary: argument parsing, overrides, atomic writes, exit codes, reports | `clap`, `tempfile`, `anyhow` |

The spec proposed a separate `halftone-grid` and `halftone-analysis`. The grid
was merged into `halftone-core` (too small alone), and analysis lives in the
`halftone` library so that `analyze`, `generate` and `convert` share one code
path (S§4.4) for the CLI and any other application.

## Generation pipeline

```text
config ─► validate ─► check source hash
       ─► decode ─► linear luminance (row-parallel; decode buffer dropped) ─► orientation
       ─► page layout (paper − printer margin; mark band; contain fit)
       ─► downscale to 2 px/cell (box filter = cell-area average)
       ─► lattice (pitch 25.4/LPI, angle, phase at image centre, whole cells only)
       ─► per cell: bilinear luminance → tone curve → coverage → diameter (exact periodic inverse)
       ─► drop marks below min_dot_mm
       ─► geometry: isolated marks emitted directly; overlapping marks unioned (i_overlay, integer µm)
       ─► SVG (one compound path + registration targets) ─► validate ─► atomic write
```

Measured on A3 at 45 LPI from a 60 MP 16-bit TIFF (Intel i9-9900K): 1.8 s and
738 MB peak for a typical image; 6.0 s for a shadow-heavy image. Peak memory
is dominated by decoding (about 10–12 bytes per input pixel).

## Concurrency and determinism

- Rayon for row-parallel luminance, lattice rows and per-cell work; ordered
  collects keep output order fixed.
- `i_overlay`'s internal threading for the union; output is identical with it on or off.
- Integer µm geometry: the same inputs produce byte-identical SVG regardless of
  thread count (tested). Cross-platform byte identity is not yet verified (D27).
- `--threads N` sets the global Rayon pool.

## Safety

- Pixel, cell and union-vertex limits are checked before large allocations
  (defaults sized for 2 GB peak; `halftone_types::limits`).
- No `unsafe` code (`unsafe_code = "forbid"` workspace-wide).
- Outputs are written to a temporary file and renamed, so a failed run never
  leaves a partial SVG.
- Every SVG is validated (structure, size, colour, path data inside the page)
  before it is written.
