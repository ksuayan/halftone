# Halftone — TODO

Implementation plan for [Halftone.md](Halftone.md) (spec, Draft 0.1),
informed by [Halftone Algos.md](Halftone%20Algos.md) (algorithm research).
Section references: **S§** = spec, **A§** = Algos.

Status: `[ ]` open · `[~]` in progress · `[x]` done · `[-]` dropped/deferred

**Progress (2026-11-18):** Phase 0 experiments A–D done ([report](Phase0-Report.md)); Phases 1–4b implemented and tested (51 tests, end-to-end `halftone convert` working). Waiting on Kyo: Exp E print tests, Affinity/Inkscape import check. Next: Phase 5–6.

---

## Product decisions (2026-11-17)

| Topic | Decision | Implications |
| --- | --- | --- |
| Output device | 600 dpi laser printer, positive on transparency film | One device pixel ≈ 42.3 µm. Ink must be pure `#000000`, because any gray gets screened by the printer. Printed with **Fit to Page** (see below). Minimum reliable dot on film must be measured (hypothesis: ≥ 3 px ≈ 0.13 mm). At 22 LPI a cell is about 27 px wide (about 740 tone levels), so the printer resolution doesn't limit tone. |
| Target apps | Affinity Designer and Inkscape; standard SVG, no binary data | Test import early: physical units (`in`/`mm`), `viewBox` scale, fill rule, holes, arc commands, and large-path performance. No embedded rasters or fonts. |
| Size target | 9 × 12 in and A3 (297 × 420 mm) | At 22 LPI: about 52k cells (9 × 12) and about 94k (A3). At 45 LPI: about 220k and 390k. Benchmarks and limits are sized to A3 at 45 LPI as the upper case. Both sizes exceed Letter, so this needs a tabloid or A3 laser printer. |
| Registration marks | **Industry-standard printer's registration targets, off by default** (changed from on, 2026-11-18); `--registration-marks` on analyze/convert, `--set-registration-marks true\|false` on generate | Standard target: concentric circle plus crosshair, centered on each of the four edges, outside the image area. Geometry follows the usual print-application defaults (≈ 6 pt offset from the image, thin hairline), but the stroke is at least 0.5 pt (≈ 4 device px at 600 dpi) so it holds on laser film. Same ink as the artwork. Written as a separate `<path id="registration-marks">` so the halftone stays one compound path and the marks are easy to remove in Affinity or Inkscape. The target spacing is known, so it also checks Fit to Page scale. Crop/trim marks are a possible separate flag (off by default) for later. |
| Memory | **Must run comfortably on an 8 GB Apple machine** | Peak resident memory ≤ 2 GB for every benchmark case including A3 at 45 LPI, leaving room for macOS and Affinity/Inkscape open at the same time. Hard safety stop well below 4 GB. Drives the max-input-pixels limit (D32) and the early downscale. |
| Preview | Yes, PNG | `--preview <file.png>`, rasterized from the final geometry with `tiny-skia`, so the preview matches the SVG exactly. |
| Print scaling | **Fit to Page** | Fit to Page rescales the artwork to the printable area, which changes the effective LPI and dot sizes. Mitigation: make the SVG canvas match the target paper's printable area (paper size minus a configurable `printer_margin`), so Fit to Page scales by about 1.0. Report the expected scale and effective LPI, and warn when the scale differs from 1.0 by more than a tolerance (e.g. 2%). The registration marks double as a check of the actual scale. |
| Platforms | macOS first; Linux and Windows if practical | CI matrix on all three; release binaries for macOS first. |
| Dependencies | **Pure Rust.** C/C++ (or other) only with a compelling, documented justification | Favors `moxcms` over `lcms2` and `i_overlay` over `clipper2`. Any exception is recorded here with the reason. |
| License | **MIT** (switched from GPL-3.0 on 2026-11-17) | `LICENSE` replaced. Every dependency must be MIT-compatible; enforce with `cargo deny`. |
| Geometry interchange | **`geo-types`** (2026-11-18) | The shared geometry type across crates and in public APIs, so other Rust libraries (`geo`, `rstar`, …) can consume our output directly. Integer µm coordinates (`MultiPolygon<i32>`), y down. Deliberate exception to S§14.1. `kurbo` stays a local helper for curves (registration marks) only. See D33. |
| Performance | **5–10 min hard ceiling** for the largest target (A3 at 45 LPI) | A ceiling, not the goal: benchmarks still report stage timings, and any stage near the ceiling is a bug. |
| Downscaling | **Scale the photo to the target output size before conversion** | Order: decode → orientation → alpha → ICC → linear light → **downscale** → tonal response → sampling. Downscale in linear light (correct averaging; also anti-aliases against moiré). Target resolution comes from the output size and LPI (D22). |
| Edge policy | **Whole cells only** (default) | Draw only cells that fall entirely inside the image area; no clipped half-dots. Future option: edge trimming (clip to the exact image rectangle). |

---

## Open decisions

Each decision is owned by the experiment or phase that resolves it. Record
the outcome and evidence here, then update the spec.

| # | Decision | Resolved by | Outcome |
| --- | --- | --- | --- |
| D1 | Default screen angle (22.5° candidate) — S§21.1, A§6.6 | Exp E (print test) | |
| D2 | Default LPI for 110 mesh (22 conservative, 22–27.5 range) — S§21.2, A§6.3 | Exp E | |
| D3 | LPI interpretation: relation to pitch, rotation, canvas — S§7.2, S§21.3 | Exp B | **Decided:** LPI along lattice axes, pitch 25.4/LPI mm; phase origin at image center; record (i, j) index ranges plus cell count, not rows/columns ([Phase0-Report](Phase0-Report.md) §B) |
| D4 | Luminance model / working color space / untagged-ICC fallback; `moxcms` preferred (pure Rust), `lcms2` only if compelling — S§5.4, S§6.2, A§5, A§7.8 | Exp A | **Decided:** `moxcms`; linearize in the image's own primaries (matrix-shaper), Y from its colorants normalized to white = 1; LUT profiles via linear sRGB; unusable/gray/untagged → sRGB with warning (§A.2) |
| D5 | Alpha default (composite on white?) and extra modes — S§5.3 | Exp A | **Decided:** composite over white in linear light; no extra modes in v1 (§A.3) |
| D6 | Geometry engine: `i_overlay` vs `clipper2` (pure-Rust preference: `clipper2` only if compelling) — S§15.2, A§7.4–7.5 | Exp D | **Decided:** `i_overlay` 9.2 (pure Rust); `clipper2` not needed (§D) |
| D7 | Circle representation: polygon vs curve-aware — S§21.7, A§8.2 | Exp D | **Decided:** area-preserving polygons; exact arcs rejected (renderer flattening loses 1–2.4% dot area) (§D.4) |
| D8 | Polygon tolerance (physical radial error) — A§8.3 | Exp D + Exp E | Tentative: 10 µm (tone unchanged, 0.94 GB worst case); confirm visually in Exp E (§D.3) |
| D9 | Integer vs float geometry; integer scale — A§8.5 | Exp D | **Decided:** integer geometry, 1 unit = 1 µm (§D.2) |
| D10 | Union batching and parallelism (all-at-once vs engine threads vs tiled `rayon::join` tree) — A§8.4, A§12.2 | Exp D | **Decided:** union only overlapping marks, `i_overlay` threads on; tiled union rejected (2–3× slower) (§D.1) |
| D11 | Simplification tolerance — S§21.8 | Phase 4 | **Decided:** no simplification; tolerance applied when building polygons |
| D12 | Sampling: cell-center bilinear vs cell-area average — A§1 | Exp B | **Decided:** center bilinear sample on the 2 px/cell downscaled plane; it matches cell-area average better than full-res sampling (§A.4) |
| D13 | Spot-growth function: LUT vs analytic — A§4.2 | Exp C | **Decided:** closed-form periodic coverage + bisection inverse; no LUT (§C) |
| D14 | Path emitted as line segments vs curve commands — A§15 | Exp D | **Decided:** line segments only (§D.4) |
| D15 | Canonical internal unit (mm vs µm) — S§9.1, A§1 | Phase 1 | **Decided:** config in mm (`f64`), geometry in µm (`i32`) |
| D16 | Linocut controls in v1 — S§21.9, S§11 | Phase 5 | |
| D17 | Paper-size catalog for v1 — S§21.10 | Phase 1 | **Decided:** letter, legal, tabloid, 9x12, a5, a4, a3 |
| D18 | Crop mode in v1? Anchor options — S§9.3, S§21.11 | Phase 3 | **Decided:** contain only in v1; crop deferred |
| D19 | Generation needs source file vs cached data; source hash — S§12.4, S§21.12 | Phase 3 | **Decided:** no cached image data; generate re-decodes the source and warns when its BLAKE3 hash changed |
| D20 | SVG via crate vs hand-written serializer — S§15.3 | Phase 4 | **Decided:** hand-written serializer; µm integer `viewBox` |
| D21 | Final crate boundaries (merge any tiny crates?) — S§14.2 | Phase 0 close | **Decided:** 7 crates — types, image, core (incl. grid), geometry, svg, `halftone` pipeline library, cli ([architecture.md](architecture.md)) |
| D22 | Downscale target: pixels per cell (or a ppi floor) at the output size — A§7.8, A§12.3 | Exp A | **Decided:** 2 px per cell, box filter, after pre-processing (§A.4) |
| D23 | Global allocator: system vs `mimalloc` — A§7.6 | Phase 6 | |
| D24 | Hybrid geometry: isolated circles (`d < p`) as exact SVG arcs, union only overlapping clusters | Exp D | **Decided:** yes for union scope (5× faster on typical images); no for arcs (§D.4) |
| D25 | Shadows above 50%: accept filled-in holes, cap maximum coverage, or switch dot shape | Exp E | |
| D26 | Source smoothing/sharpening before sampling (moiré from textured or printed sources) | Exp A/B | **Decided:** no separate filter; the 2 px/cell box downscale is the prefilter (§A.4) |
| D27 | Determinism scope: same machine and version, or byte-identical across platforms | Exp D (ties to D9) | Same machine: identical across runs and thread counts. Cross-platform: untested (needs Apple Silicon run) (§D.2) |
| D28 | Performance and memory targets | Kyo | **Decided:** 5–10 min hard ceiling for A3 at 45 LPI; peak ≤ 2 GB so it runs on an 8 GB Apple machine |
| D29 | Layout: margin size for marks, `printer_margin` for Fit to Page, edge policy | Phase 3 | **Decided:** whole cells only (trimming deferred); standard registration targets, off by default. Still open: exact margin |
| D30 | License | Kyo | **Decided:** MIT |
| D31 | Config source path: relative to the JSON or absolute | Phase 3 | **Decided:** relative to the config folder when inside it, else absolute |
| D32 | Safety-limit defaults: max input pixels, cells, vertices (sized to the 2 GB budget) | Phase 1 | **Decided:** 150 MP input, 2 M cells, 6 M union vertices (`halftone_types::limits`) |
| D33 | Geometry interchange type: `kurbo` vs `geo-types` vs own types | Kyo | **Decided:** `geo-types` 0.7 as the public geometry type. Rationale: matches our data (polygons with holes), `geo`'s Boolean ops run on `i_overlay` (our engine), stable 0.7 API, `rstar` integration; `Coord<i32>` is 8 bytes, same as `i_overlay`'s points. Conventions: 1 unit = 1 µm, y down (exterior rings appear clockwise on screen, holes counter-clockwise; `geo` algorithms that assume y-up orientation must account for this). `kurbo` rejected as foundation (curve-first, `f64` only, no polygon-with-holes, ~56-byte path elements). |

---

## Phase 0 — Research and prototypes (S§20, A§13)

A throwaway prototype workspace ([`prototypes/`](../prototypes)) that produces evidence
for the decisions above. Not shipped code. Results: [Phase0-Report.md](Phase0-Report.md) (2026-11-18).

### Experiment A — Image and color pipeline → D4, D5

- [x] Decode JPEG, PNG, TIFF (8- and 16-bit) with `image`
- [x] Read EXIF orientation and ICC metadata; confirm what `image` exposes
- [-] Convert via `lcms2` to a working RGB space; measure cost — not needed: `moxcms` covers all tested profiles (pure-Rust rule)
- [-] Compare `moxcms` (pure Rust) against `lcms2` — `moxcms` chosen without an `lcms2` comparison (pure-Rust rule)
- [x] Produce a linear-light luminance plane (`0.0` = black, `1.0` = white)
- [x] Downscale in linear light with `fast_image_resize` to a resolution derived from output size and LPI; find the smallest pixels-per-cell factor whose sampling error stays within tolerance (D22)
- [x] Verify against known color patches and grayscale ramps
- [x] Compare alpha compositing on white vs other backgrounds
- [x] Write up: decoder choice, ICC strategy, internal pixel representation, analysis resolution

### Experiment B — AM screen geometry → D3, D12

- [x] Physical Cartesian lattice from LPI (pitch = 25.4 mm / LPI)
- [x] Rotation plus inverse transform; stable screen phase/origin (A§9.1)
- [x] Enumerate cells that cover the rotated canvas; edge behavior (A§9.3)
- [x] Map physical coordinates to source pixels (A§9.2)
- [x] Cell-center bilinear sampling, compared with cell-area average
- [-] Preview SVG of a grayscale ramp, without union — superseded by exp_d/exp_e outputs
- [x] Write up: angle semantics, LPI convention, row/column definition for rotated grids

### Experiment C — Coverage calibration → D13

- [x] Isolated-circle mapping `d = p·√(4C/π)`
- [x] Periodic union coverage for `d ∈ [0, √2·p]`
- [-] Monotonic coverage→diameter LUT with interpolation — closed form + exact inverse instead
- [x] Check prediction against rasterized coverage; report max error

### Experiment D — Boolean geometry → D6, D7, D8, D9, D10, D14

- [-] Same circle sets for `i_overlay` and `clipper2` — `clipper2` skipped (C++; `i_overlay` sufficient)
- [x] Cases: disjoint, tangent, overlapping, dense, holes, clipped at the canvas edge
- [x] Segment count from radial-error tolerance (A§8.3)
- [-] Integer and float modes — integer µm chosen; float mode not needed
- [x] All-at-once vs engine multithreading vs tiles unioned in parallel and merged in a fixed `rayon::join` tree (A§12.2)
- [x] Measure runtime, peak memory, vertex count, SVG size
- [x] Check licenses and maintenance health
- [~] Render holes in at least two independent SVG renderers (A§8.7); use `resvg` to check filled area automatically — `resvg` done; second renderer = Affinity/Inkscape import test
- [x] Prototype hybrid output: exact arcs for isolated circles, union only for overlapping clusters (D24)
- [ ] Import test files (9 × 12 and A3, 22 and 45 LPI) into **Affinity Designer** and **Inkscape**: physical size, holes, arcs, load and edit time (D7, D14, D24)
- [ ] Check float output on Apple Silicon vs x86 Linux (D27)

### Experiment E — Print test chart → D1, D2, D8 (needs physical printing)

- [x] Chart generator (`exp_e letter|tabloid|a3`) with wedges, shadow ramps, min-dot ramp, angles, scale bars, registration targets
- [ ] Charts for 110 mesh at 22, 24 and 27.5 LPI, printed on the 600 dpi laser printer to transparency with Fit to Page; measure scale bar and target spacing
- [ ] Dot-size ramp in device pixels (1–6 px) to find the minimum dot that holds on film and through exposure
- [ ] Shadow ramp (50–100%) to see where holes fill in (D25)
- [ ] Check toner density/opacity on film; note whether a densifier is needed
- [ ] Angle variants: 15°, 22.5°, 30°, 45°
- [ ] Tonal ramp and minimum-feature ramp
- [ ] Expose and print; record which marks survive

### Phase 0 close

- [x] Final dependency list with versions to pin: root `Cargo.toml` `[workspace.dependencies]`; MSRV 1.89 (from `moxcms`/`i_overlay`)
- [x] Confirm every dependency license against the D30 choice with `cargo deny`; prefer pure-Rust dependencies for cross-platform builds — `deny.toml`; all licenses permissive
- [x] Experiment A: measure peak memory of decoding the largest expected inputs (e.g. 60 MP 16-bit TIFF); strip-wise TIFF decoding not needed
- [x] Confirm no C/C++ dependencies remain — `blake3` built with `pure`; its `cc` build-dependency compiles nothing
- [x] Final crate graph (D21) — [architecture.md](architecture.md)
- [ ] Benchmark plan (S§16.4, A§12.4) — stage timings exist (`-v`); `criterion` suite still to write (Phase 6)
- [ ] Update spec to Draft 0.2 with resolved decisions, including the S§14.1 exception for `geo-types` (D33) — decisions are recorded here and in [architecture.md](architecture.md); spec text not yet revised

**Exit:** met (2026-11-18).

---

## Phase 1 — Workspace and foundational types (S§14, S§20) — done 2026-11-18

- [x] Cargo workspace: root `Cargo.toml`, `rust-toolchain.toml` (1.97.1), `Cargo.lock`, `rustfmt.toml`
- [x] Crates per D21: `halftone-types`, `-image`, `-core`, `-geometry`, `-svg`, `halftone` (pipeline library), `-cli`
- [x] Dependency direction (S§14.4): libraries → `halftone-types` only; `halftone` orchestrates; no cycles
- [x] `halftone-types`:
  - [x] Units (D15): config in mm (`f64`, `_mm` names), geometry in µm (`i32`); `in`/`cm` only at the CLI boundary
  - [x] Angle normalization to [0, 90)
  - [x] Paper catalog: letter, legal, tabloid, 9x12, a5, a4, a3 (D17)
  - [x] Safety-limit defaults (D32): 150 MP input, 2 M cells, 6 M union vertices
  - [x] Geometry types (D33): `geo_types` re-exports with documented conventions
  - [x] Config structs (serde + schemars, `deny_unknown_fields`)
  - [x] Errors: `thiserror` in libraries, `anyhow` in the CLI
- [x] `unsafe_code = "forbid"` workspace-wide
- [~] CI: `.github/workflows/ci.yml` (fmt, clippy `-D warnings`, test, rustdoc on macOS/Linux/Windows) + `cargo deny` — written, not yet run on GitHub; plain `cargo test` instead of `nextest`
- [x] Unit tests: unit conversions (proptest round trips), angle normalization, colours
- [x] [architecture.md](architecture.md)

---

## Phase 2 — Image ingestion and tonal analysis (S§5, S§6) — done 2026-11-18

### `halftone-image`

- [x] JPEG / PNG / TIFF decode behind a format-agnostic API (`probe`, `load_luminance`)
- [x] Pixel limit checked from the header **before** decoding
- [x] EXIF orientation (on the luminance plane; all 8 cases tested against `image`)
- [x] RGB, gray, RGBA, gray+alpha; 8- and 16-bit
- [x] Alpha composited over `tone.analysis_background` (default white)
- [x] ICC per D4; unusable profiles fall back to sRGB with a warning
- [x] Metadata: size after orientation, orientation, pixel format, colour path
- [x] No `image`/`moxcms` types in the public API

### `halftone-core`

- [x] Linear-light luminance: 65 536-entry LUT, row-parallel (in `halftone-image`)
- [x] Downscale per D22 after pre-processing, before tone and sampling
- [x] Tone response: black point, white point, gamma, invert, max coverage; monotonic (proptest)
- [-] Contrast/brightness controls — dropped for v1: black/white points and gamma cover them without overlapping controls (S§6.3 "avoid ambiguous controls")
- [x] Coverage → diameter: closed-form periodic coverage, exact inverse (D13)
- [ ] Optional `print_compensation` hook (A§4.3) — after Exp E
- [x] Tests: boundaries, inversion, monotonicity, brute-force coverage check

---

## Phase 3 — Grid, recommendations and configuration (S§7–9, S§12) — done 2026-11-18

### Lattice (`halftone-core::lattice`)

- [ ] Grid strategy trait for future hex/stochastic grids (S§7.3) — Cartesian only so far; add the trait with the second grid type
- [x] Rotation, phase at image centre, coverage of the image area
- [x] Physical ↔ source mapping (bilinear on the working plane)
- [x] Contain fit, centred, recorded in `layout.image`; paper orientation follows the photo unless `--orientation` is given
- [-] Crop mode (D18) — deferred; contain only in v1
- [x] Page layout: paper − `printer_margin_mm` per side; 12 mm mark band (D29)
- [x] Edge policy: whole cells only (cell kept if a max-size dot fits inside the image)
- [~] Fit to Page: page = printable area so scale ≈ 1 by construction; analysis reports paper vs page size. Explicit scale/effective-LPI warning not added (needs the measured printer margin)
- [x] Index ranges/cell count per D3 (no rows/columns)
- [x] Tests: whole cells inside, inverse round trip, determinism, angles 0–89.9°

### Analysis (`halftone` library)

- [x] LPI = mesh / ratio (ratio 5.0 default, recorded in JSON); range mesh/5..mesh/4 recorded
- [x] Explicit `--lpi` wins; warning when finer than mesh/4
- [x] Minimum dot (0.13 mm screen-print, 0.5 mm linocut) and coverage cap in physical units
- [ ] Warn when the tonal range implies dots below the minimum (A§6.5)
- [x] `screen-print` profile (110 mesh, 22 LPI, 22.5°) — provisional until Exp E; `linocut` provisional (D16)
- [x] Recommendations, user-set fields and effective values kept separate
- [x] `schema_version`, `app_version`, source metadata, `blake3` hash (D19: no cached image data; generate re-decodes and warns on hash mismatch)
- [ ] Mesh thread diameter / opening fields (A§6.4) — not needed by v1 recommendations
- [x] `serde_path_to_error` field-path errors
- [x] Strict validation (S§12.3); every rule names field, value, expected range
- [x] Source path relative to the config folder when inside it, else absolute (D31)
- [x] Tests: JSON round trip, unknown field, invalid values, limits, changed source
- [x] [configuration.md](configuration.md) + [config.schema.json](config.schema.json) (`halftone schema`)

---

## Phase 4 — Geometry and SVG (S§10, S§19) — done 2026-11-18

### `halftone-geometry`

- [x] Output `geo_types::MultiPolygon<i32>` (D33); `i_overlay` conversions internal; rings closed
- [x] `geo` validity test (no overlaps, holes inside exteriors) and area agreement
- [x] Area-preserving circle polygons, 10 µm tolerance (D7/D8), deterministic vertices
- [x] Suppress marks below `tone.min_dot_mm`
- [-] Clip to canvas — not needed: whole-cell policy keeps every dot inside the image
- [x] Union only overlapping marks; overlap test uses polygon vertex radii (fixes a Phase 0 prototype bug found by the `geo` validity test)
- [-] Simplification (D11) — not needed: tolerance is applied at construction
- [x] Complexity limit (`max_union_vertices`) fails loudly
- [x] Tests: disjoint, overlapping with holes, mixed ramp, thread-mode determinism, limit

### `halftone-svg`

- [x] Hand-written serializer (D20): one compound `<path>`, `#RRGGBB` fill, `evenodd`
- [x] Physical `width`/`height` in mm; `viewBox` in integer µm (no float formatting, so no `ryu`)
- [x] Transparent background, no raster, no metadata
- [x] Registration targets: circle + crosshair on each edge in the mark band, 0.5 pt stroke, ink colour, `<path id="registration-marks">`
- [x] `output.registration_marks` (default `false` since 2026-11-18; was `true`); `--registration-marks` on analyze/convert, `--set-registration-marks true|false` on generate; same in `scripts/test-run.sh` (`MARKS=1`)
- [ ] Check marks and artwork in Affinity Designer and Inkscape (Kyo)
- [x] Warning when ink is not pure black
- [x] Output validator (S§19); every generated SVG is validated before writing
- [x] `resvg` render matches geometric area; PNG preview flattened on white

---

## Phase 4b — CLI (S§4, S§13) — done 2026-11-18

- [x] `analyze`, `generate`, `convert`, `validate`, `schema`, `version`
- [x] `analyze` options incl. `--paper`, `--width/--height/--units`, `--orientation`, `--profile`, `--mesh`, `--lpi`, `--angle`, `--ink`, `--printer-margin-mm`, `--invert`
- [x] `generate` overrides (`--set-lpi`, `--set-angle`, `--set-ink`, `--min-dot-mm`, `--max-coverage`, `--gamma`, `--black-point`, `--white-point`, `--tolerance-um`, `--set-registration-marks`), validated by the same code as JSON
- [x] `convert` = `analyze` + `generate` (byte-identical output, tested)
- [x] `--verbose`: effective settings, user-set fields, geometry stats, stage timings
- [x] Actionable errors; warnings printed separately
- [x] Exit codes: 2 usage, 3 input, 4 config, 5 processing, 6 output, 70 internal
- [x] Atomic writes (`tempfile` persist); no file left on failure (tested)
- [-] Ctrl-C handler — not needed for correctness: atomic writes already prevent partial files
- [x] `--threads`
- [ ] `tracing` spans / `indicatif` progress — runs finish in seconds; revisit if needed
- [x] `--preview FILE.png [--preview-dpi]`, `--save-config FILE.json`
- [x] `validate CONFIG [--svg FILE]`

---

## Phase 5 — Print-production controls (S§8, S§11)

- [ ] Finalize the `screen-print` preset from Exp E results
- [ ] Minimum-feature-width check (A§10.1)
- [ ] Minimum-gap check, kept distinct from feature width (S§11, A§10.2)
- [ ] `linocut` preset plus the v1 controls chosen in D16
- [ ] Print test-chart generator command (A§6.6, A§11.6) — port `prototypes` `exp_e` into the CLI
- [ ] More complete diagnostics for print-risky settings

**Exit:** settings and output are fit for controlled print-production tests.

---

## Phase 6 — Performance and hardening (S§16, S§17)

- [ ] `criterion` benchmarks for each stage (S§16.1) on the S§16.4 matrix
- [x] End-to-end check: A3 at 45 LPI from a 60 MP 16-bit TIFF: 1.8 s typical, 6.0 s shadow-heavy (D28) — measured manually
- [~] Peak memory: 738 MB whole-process for that case (`/usr/bin/time -l`); per-stage `dhat` not done
- [ ] Memory gate automated in CI (≤ 2 GB)
- [x] Rayon for per-cell work; `i_overlay` threads for the union (D10)
- [x] Determinism: identical SVG with and without union threads (tested)
- [ ] Benchmark `mimalloc` as the global allocator (D23)
- [ ] `smallvec` / buffer reuse only where `dhat` shows allocator pressure
- [~] Geometry stress tests — dense/overlapping/disjoint unit tests; no large stress suite yet
- [ ] Licensed photo fixture set in `tests/fixtures/` (tests currently generate synthetic images)
- [x] `scripts/test-run.sh`: batch-convert `prototypes/photos` (SVG, preview, config, log per photo); 6/6 of Kyo's photos convert in 2–8 s each (2026-11-18)
- [x] `scripts/test-run.sh` exposes page, screen, tone and geometry options as flags or environment variables; defaults letter / 110 mesh. 27/27 photos convert in 1–2 s each
- [x] ICC profile names shown as plain text in `source.color` (was a debug dump; found by the test run)
- [x] `.gitignore` covers build output, local photos, generated output, OS and editor files; `CLAUDE.md` added with agent instructions (2026-11-18)
- [x] [CLI.md](CLI.md): every command, option, accepted value, default, limit and exit code; missing `--help` texts filled in
- [x] Whole-cell rule uses the drawn polygon's outer radius (+1 µm), not the circle's: with marks off the image touches the page edge and full-size dots could poke 2–7 µm past it (caught by the SVG validator on a dark scan; regression test added, 2026-11-18)
- [x] Image areas too small for one whole screen cell now fail with exit 5 (previously wrote an empty SVG; found while writing CLI.md)
- [x] Integration tests (S§17.3): mesh-derived and explicit LPI, overrides, orientation, uniform-gray coverage, exit codes
- [x] Regression checks on structure, size, area and coverage rather than bytes (except same-version determinism)
- [ ] Rasterized visual regression comparisons (A§11.5)
- [~] CI matrix: macOS, Linux, Windows — workflow written, not yet run
- [~] README usage done; `docs/algorithms.md` and release packaging not yet
- [ ] Print workflow guide (Fit to Page with matching `printer_margin`, no toner saving, darkest density, scale check with targets)

**Exit:** performance measured, major failure modes covered, release behavior documented.

---

## Release checklist (S§22)

- [x] 1. Accepts JPEG/PNG/TIFF
- [x] 2. Paper size or custom dimensions
- [x] 3. Analyze writes JSON config
- [x] 4. Grid size plus mesh-derived LPI recommendation
- [x] 5. Edit JSON or override from the CLI
- [x] 6. Cartesian grid of variable circles
- [x] 7. Dark areas give larger circles
- [x] 8. Normal and inverted mapping
- [x] 9. Transparent SVG with a configurable ink color
- [x] 10. Compound path from Boolean union
- [x] 11. Config and SVG validation
- [~] 12. Test suite (51 tests) — stage benchmarks still to write

---

## Deferred (non-goals for v1, S§2.3)

- [-] Multi-ink / CMYK separations
- [-] Hex, isometric, stochastic and adaptive grids (keep the trait open)
- [-] Non-circle marks
- [-] GUI
- [-] Print calibration workflow (measured dot-gain curves)
