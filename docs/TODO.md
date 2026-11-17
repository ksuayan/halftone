# Halftone — TODO

Implementation plan for [Halftone.md](Halftone.md) (spec, Draft 0.1),
informed by [Halftone Algos.md](Halftone%20Algos.md) (algorithm research).
Section references: **S§** = spec, **A§** = Algos.

Status: `[ ]` open · `[~]` in progress · `[x]` done · `[-]` dropped/deferred

---

## Open decisions

Each decision is owned by the experiment or phase that resolves it. Record
the outcome and evidence here, then update the spec.

| # | Decision | Resolved by | Outcome |
| --- | --- | --- | --- |
| D1 | Default screen angle (22.5° candidate) — S§21.1, A§6.6 | Exp E (print test) | |
| D2 | Default LPI for 110 mesh (22 conservative, 22–27.5 range) — S§21.2, A§6.3 | Exp E | |
| D3 | LPI interpretation: relation to pitch, rotation, canvas — S§7.2, S§21.3 | Exp B | |
| D4 | Luminance model / working color space / untagged-ICC fallback; `lcms2` vs `moxcms` — S§5.4, S§6.2, A§5, A§7.8 | Exp A | |
| D5 | Alpha default (composite on white?) and extra modes — S§5.3 | Exp A | |
| D6 | Geometry engine: `i_overlay` vs `clipper2` — S§15.2, A§7.4–7.5 | Exp D | |
| D7 | Circle representation: polygon vs curve-aware — S§21.7, A§8.2 | Exp D | |
| D8 | Polygon tolerance (physical radial error) — A§8.3 | Exp D + Exp E | |
| D9 | Integer vs float geometry; integer scale — A§8.5 | Exp D | |
| D10 | Union batching and parallelism (all-at-once vs engine threads vs tiled `rayon::join` tree) — A§8.4, A§12.2 | Exp D | |
| D11 | Simplification tolerance — S§21.8 | Phase 4 | |
| D12 | Sampling: cell-center bilinear vs cell-area average — A§1 | Exp B | |
| D13 | Spot-growth function: LUT vs analytic — A§4.2 | Exp C | |
| D14 | Path emitted as line segments vs curve commands — A§15 | Exp D | |
| D15 | Canonical internal unit (mm vs µm) — S§9.1, A§1 | Phase 1 | |
| D16 | Linocut controls in v1 — S§21.9, S§11 | Phase 5 | |
| D17 | Paper-size catalog for v1 — S§21.10 | Phase 1 | |
| D18 | Crop mode in v1? Anchor options — S§9.3, S§21.11 | Phase 3 | |
| D19 | Generation needs source file vs cached data; source hash — S§12.4, S§21.12 | Phase 3 | |
| D20 | SVG via crate vs hand-written serializer — S§15.3 | Phase 4 | |
| D21 | Final crate boundaries (merge any tiny crates?) — S§14.2 | Phase 0 close | |
| D22 | Analysis resolution: downsample factor per cell vs full-resolution sampling — A§7.8, A§12.3 | Exp A | |
| D23 | Global allocator: system vs `mimalloc` — A§7.6 | Phase 6 | |

---

## Phase 0 — Research and prototypes (S§20, A§13)

A throwaway prototype workspace (e.g. `prototypes/`) that produces evidence
for the decisions above. Not shipped code.

### Experiment A — Image and color pipeline → D4, D5

- [ ] Decode JPEG, PNG, TIFF (8- and 16-bit) with `image`
- [ ] Read EXIF orientation and ICC metadata; confirm what `image` exposes
- [ ] Convert via `lcms2` to a working RGB space; measure cost
- [ ] Compare `moxcms` (pure Rust) against `lcms2`
- [ ] Produce a linear-light luminance plane (`0.0` = black, `1.0` = white)
- [ ] Downsample with `fast_image_resize` to a cell-relative resolution; measure time, peak memory and sampling error (D22)
- [ ] Verify against known color patches and grayscale ramps
- [ ] Compare alpha compositing on white vs other backgrounds
- [ ] Write up: decoder choice, ICC strategy, internal pixel representation, analysis resolution

### Experiment B — AM screen geometry → D3, D12

- [ ] Physical Cartesian lattice from LPI (pitch = 25.4 mm / LPI)
- [ ] Rotation plus inverse transform; stable screen phase/origin (A§9.1)
- [ ] Enumerate cells that cover the rotated canvas; edge behavior (A§9.3)
- [ ] Map physical coordinates to source pixels (A§9.2)
- [ ] Cell-center bilinear sampling, compared with cell-area average
- [ ] Preview SVG of a grayscale ramp, without union
- [ ] Write up: angle semantics, LPI convention, row/column definition for rotated grids

### Experiment C — Coverage calibration → D13

- [ ] Isolated-circle mapping `d = p·√(4C/π)`
- [ ] Periodic union coverage for `d ∈ [0, √2·p]`
- [ ] Monotonic coverage→diameter LUT with interpolation
- [ ] Check prediction against rasterized coverage; report max error

### Experiment D — Boolean geometry → D6, D7, D8, D9, D10, D14

- [ ] Same circle sets for `i_overlay` and `clipper2`
- [ ] Cases: disjoint, tangent, overlapping, dense, holes, clipped at the canvas edge
- [ ] Segment count from radial-error tolerance (A§8.3)
- [ ] Integer and float modes
- [ ] All-at-once vs engine multithreading vs tiles unioned in parallel and merged in a fixed `rayon::join` tree (A§12.2)
- [ ] Measure runtime, peak memory, vertex count, SVG size
- [ ] Check licenses and maintenance health
- [ ] Render holes in at least two independent SVG renderers (A§8.7); use `resvg` to check filled area automatically

### Experiment E — Print test chart → D1, D2, D8 (needs physical printing)

- [ ] Charts for 110 mesh at 22, 24 and 27.5 LPI
- [ ] Angle variants: 15°, 22.5°, 30°, 45°
- [ ] Tonal ramp and minimum-feature ramp
- [ ] Expose and print; record which marks survive

### Phase 0 close

- [ ] Final dependency list with versions to pin; verify the A§7.8 candidates (maintenance, license, MSRV)
- [ ] Final crate graph (D21)
- [ ] Benchmark plan (S§16.4, A§12.4)
- [ ] Update spec to Draft 0.2 with resolved decisions

**Exit:** major dependencies and algorithms selected on evidence.

---

## Phase 1 — Workspace and foundational types (S§14, S§20)

- [ ] Cargo workspace: root `Cargo.toml`, `rust-toolchain.toml`, `Cargo.lock`
- [ ] Crate skeletons: `halftone-types`, `-image`, `-core`, `-grid`,
      `-analysis`, `-geometry`, `-svg`, `-cli` (adjust per D21)
- [ ] Enforce dependency direction (S§14.4); no cycles
- [ ] `halftone-types`:
  - [ ] Physical length type with canonical unit (D15); `in`/`cm` conversion only at boundaries
  - [ ] Pixel dimensions, luminance, angle (degrees, normalized)
  - [ ] Paper-size catalog: Letter, Legal, Tabloid, A3, A4, A5 (D17), easy to extend
  - [ ] Config structs (serde), explicit field names with units (S§3)
  - [ ] Error conventions: `thiserror` in libraries, `anyhow` (or `miette`) in the CLI only
- [ ] `#![forbid(unsafe_code)]` by default
- [ ] CI: build, `cargo nextest`, clippy, rustdoc, `cargo deny` (licenses, advisories)
- [ ] Unit tests: unit conversions (with a proptest round trip), angle normalization
- [ ] `docs/architecture.md` stub

**Exit:** the workspace builds, tests and documents cleanly.

---

## Phase 2 — Image ingestion and tonal analysis (S§5, S§6)

### `halftone-image`

- [ ] JPEG / PNG / TIFF decode behind a format-agnostic API
- [ ] Check dimension and allocation limits **before** allocating (S§5.2, S§18)
- [ ] Apply EXIF orientation
- [ ] Normalize RGB, gray and RGBA; 8-bit and high bit depth
- [ ] Composite alpha on the configured background (default white, per D5)
- [ ] ICC conversion per D4; document assumptions and never claim colorimetric accuracy without a profile
- [ ] Extract metadata (size, color model, alpha, orientation applied)
- [ ] Don't leak `image`/`lcms2` types through the public API

### `halftone-core`

- [ ] Linear-light luminance per D4: 256-entry sRGB→linear LUT for 8-bit input; `par_chunks_exact_mut(width)` over rows (A§12.2)
- [ ] Downsample to the analysis resolution per D22 before the float stages
- [ ] Tonal response: black point, white point, gamma, contrast, brightness;
      monotonic and deterministic (S§6.3)
- [ ] Luminance → target coverage, `dark_larger` default and inverted mode (S§6.1)
- [ ] Coverage → diameter via spot-growth function (D13, A§4.2)
- [ ] Optional `print_compensation` hook, unused by default (A§4.3)
- [ ] Tests: known RGB values, boundary values, inversion, monotonicity
      (proptest), diameter stays within [min, max]

**Exit:** deterministic luminance and tonal results on fixture images.

---

## Phase 3 — Grid, recommendations and configuration (S§7–9, S§12)

### `halftone-grid`

- [ ] Cartesian grid behind a strategy trait (room for hex/stochastic later, S§7.3)
- [ ] Rotation, explicit origin/phase, coverage of the canvas
- [ ] Physical ↔ source coordinate mapping
- [ ] Contain fit with recorded placement and margins; crop mode per D18
- [ ] Rows/columns computed per the D3 convention (not naive `width × LPI`)
- [ ] Tests: rotated placement, no overflow or negative dimensions (proptest)

### `halftone-analysis`

- [ ] LPI recommendation `mesh / ratio` (default 5.0; 4.5 and 4.0 selectable), formula recorded in JSON
- [ ] Explicit `--lpi` takes precedence; warn when outside the advisory range
- [ ] Recommend min/max diameter and suppression threshold in physical units
- [ ] Warn when the tonal range implies very small dots (A§6.5)
- [ ] `screen-print` profile (110 mesh, values from Exp E)
- [ ] Config model with **recommended**, **user-supplied** and **effective** values kept separate (A§15)
- [ ] Config records `schema_version`, app version, source metadata and (per D19) a `blake3` source hash
- [ ] Mesh as `count_per_inch` plus optional thread diameter/opening; never invented (A§6.4)
- [ ] Load config through `serde_path_to_error` so errors name the field path
- [ ] Strict validation (S§12.3): schema version, enums, units, non-finite values,
      negative values, min > max, colors, angle range, LPI/mesh ranges,
      grid consistency, missing source
- [ ] Defaults documented; no silent fallback for settings that affect geometry
- [ ] Tests: JSON round trip keeps effective settings (proptest), each validation rule
- [ ] `docs/configuration.md` plus a JSON Schema generated with `schemars` from the config types

**Exit:** `analyze` produces a valid, reusable configuration.

---

## Phase 4 — Geometry and SVG (S§10, S§19)

### `halftone-geometry`

- [ ] Library-independent geometry output type (A§8.1)
- [ ] Circle construction per D7/D8 with deterministic vertex placement
- [ ] Suppress marks below the threshold (S§6.5)
- [ ] Clip to the canvas (before or after union, per benchmarks; A§8.6)
- [ ] Primitives generated per row with `into_par_iter().flat_map_iter(...)` and an ordered `collect` (deterministic)
- [ ] Boolean union per D6/D9/D10; holes and disconnected parts preserved; fixed merge order
- [ ] Simplify only within documented tolerance (D11)
- [ ] Complexity limit: fail loudly instead of degrading (S§16.3, A§10.4)
- [ ] Tests: disjoint, tangent, overlap, full-coverage, holes; filled area within tolerance; all coordinates finite

### `halftone-svg`

- [ ] Serializer per D20: one `<path>` (compound), single fill, explicit `fill-rule`
- [ ] Physical `width`/`height` in the chosen unit plus a matching `viewBox`
- [ ] Transparent background, no raster, no extra metadata
- [ ] Compact, deterministic number formatting: round, then `ryu`
- [ ] Output validator (S§19): well-formed (`roxmltree`), dimensions, viewBox, color, one path, finite coordinates, within canvas

**Exit:** valid single-color positive SVG from every supported fixture.

---

## Phase 4b — CLI (S§4, S§13)

- [ ] `clap` subcommands: `analyze`, `generate`, `convert`, `validate`, `version`
- [ ] `analyze` options (S§13.1) including `--paper`
- [ ] `generate` overrides (S§13.2), validated by the same code as JSON
- [ ] `convert` uses the same analysis and generation APIs, no separate path (S§4.4)
- [ ] `--verbose` prints the effective config, overrides included
- [ ] Actionable errors: the setting, its value, the expected range and a fix (S§13.3)
- [ ] Distinct exit codes for input, config, processing, output and internal errors
- [ ] Warnings kept separate from errors (S§17.5)
- [ ] Atomic write via `tempfile::NamedTempFile::persist`; never a partial SVG (S§18)
- [ ] Ctrl-C handling: `ctrlc` plus an `AtomicBool` checked between stages
- [ ] `--threads` via `rayon::ThreadPoolBuilder` (fallback `RAYON_NUM_THREADS`)
- [ ] `tracing` spans per stage; `--verbose` prints stage timings
- [ ] `indicatif` progress, with `ParallelProgressIterator` on the parallel stages
- [ ] `validate` checks a config and, optionally, an existing SVG

---

## Phase 5 — Print-production controls (S§8, S§11)

- [ ] Finalize the `screen-print` preset from Exp E results
- [ ] Minimum-feature-width check (A§10.1)
- [ ] Minimum-gap check, kept distinct from feature width (S§11, A§10.2)
- [ ] `linocut` preset plus the v1 controls chosen in D16
- [ ] Print test-chart generator command (A§6.6, A§11.6)
- [ ] More complete diagnostics for print-risky settings

**Exit:** settings and output are fit for controlled print-production tests.

---

## Phase 6 — Performance and hardening (S§16, S§17)

- [ ] `criterion` benchmarks for each stage (S§16.1) on the S§16.4 matrix
- [ ] Peak-memory tracking for each stage with `dhat`
- [ ] Rayon for per-cell work; parallel union only if D10 shows it is safe and faster
- [ ] Determinism check: identical SVG at 1 thread and N threads (no float `reduce`/`sum` affecting geometry)
- [ ] Benchmark `mimalloc` as the global allocator (D23)
- [ ] `smallvec` / buffer reuse only where `dhat` shows allocator pressure
- [ ] Geometry stress tests (dense, many overlaps, mostly disjoint)
- [ ] Licensed fixture set in `tests/fixtures/`: JPEG, PNG, TIFF, gray, RGBA, portrait and landscape
- [ ] Integration tests (S§17.3): explicit LPI, mesh-derived LPI, overrides, inversion, one compound path
- [ ] Regression tests that check structure, bounding box, area and component count; no byte-for-byte checks (S§17.4)
- [ ] Rasterized visual regression comparisons with `resvg` / `tiny-skia` (A§11.5)
- [ ] `docs/algorithms.md`, README usage and release packaging

**Exit:** performance measured, major failure modes covered, release behavior documented.

---

## Release checklist (S§22)

- [ ] 1. Accepts JPEG/PNG/TIFF
- [ ] 2. Paper size or custom dimensions
- [ ] 3. Analyze writes JSON config
- [ ] 4. Grid size plus mesh-derived LPI recommendation
- [ ] 5. Edit JSON or override from the CLI
- [ ] 6. Cartesian grid of variable circles
- [ ] 7. Dark areas give larger circles
- [ ] 8. Normal and inverted mapping
- [ ] 9. Transparent SVG with a configurable ink color
- [ ] 10. Compound path from Boolean union
- [ ] 11. Config and SVG validation
- [ ] 12. Test suite plus stage benchmarks

---

## Deferred (non-goals for v1, S§2.3)

- [-] Multi-ink / CMYK separations
- [-] Hex, isometric, stochastic and adaptive grids (keep the trait open)
- [-] Non-circle marks
- [-] GUI
- [-] Print calibration workflow (measured dot-gain curves)
