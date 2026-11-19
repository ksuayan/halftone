# Halftone — agent instructions

Rust workspace that converts photos into single-ink halftone SVG positives
(screen printing first, linocut second). Owner: Kyo.

## Where things are

- `crates/` — the product (7 crates; see `docs/architecture.md`):
  `halftone-types` (units, config, validation, geometry types) →
  `halftone-image`, `halftone-core`, `halftone-geometry`, `halftone-svg` →
  `halftone` (analyze/generate pipeline) → `halftone-cli` (binary `halftone`).
- `docs/TODO.md` — the plan **and the decision log** (D1…D33). Read it before
  changing behaviour; it says what is decided, provisional, or waiting on Kyo.
- `docs/Halftone.md` (spec), `docs/Halftone Algos.md` (research),
  `docs/Phase0-Report.md` (measurements behind the design).
- `docs/CLI.md`, `docs/configuration.md`, `docs/config.schema.json` — user docs.
- `prototypes/` — throwaway Phase 0 experiments (own workspace; not the product).
  `prototypes/photos/` and `prototypes/output/` are local and git-ignored.
- `scripts/test-run.sh` — batch-convert `prototypes/photos` (see `--help`).

## Commands

```bash
cargo build --release                               # binary: target/release/halftone
cargo test --workspace                              # all tests (~1 min incl. build)
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all                                     # rustfmt.toml: max_width 120
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
scripts/test-run.sh                                 # real photos, letter / 110 mesh
```

Before saying a change is done: fmt, clippy (`-D warnings`) and the full
test suite pass. For changes to tone, geometry or SVG output, also run
`scripts/test-run.sh` and look at a preview PNG.

## Rules

- **Pure Rust.** No C/C++ dependencies unless the justification is compelling
  and recorded in `docs/TODO.md`. `blake3` stays on its `pure` feature.
- **Licenses must be MIT-compatible** (project is MIT; see `deny.toml`).
- **No `unsafe`** (`unsafe_code = "forbid"`).
- **Units:** config lengths in mm (`f64`, fields named `*_mm`); geometry in
  integer µm (`i32`), x right, y down. Convert inches/cm only at the CLI.
- **Geometry interchange is `geo-types`** `MultiPolygon<i32>` (D33). Keep
  `i_overlay` types inside `halftone-geometry`.
- **Determinism:** identical input → byte-identical SVG regardless of thread
  count. No floating-point `reduce`/`sum` that affects geometry; use ordered
  collects.
- **Dots are polygons, never SVG arcs** (renderers flatten arcs inaccurately;
  Phase 0 report §D.4). Polygons are area-preserving.
- **One code path:** `convert` must stay `analyze` + `generate`.
- **Config changes:** keep `deny_unknown_fields`, put validation in
  `halftone-types/src/validate.rs` (shared by JSON and CLI overrides), bump
  `SCHEMA_VERSION` for incompatible changes, and regenerate the schema:
  `target/release/halftone schema > docs/config.schema.json`.
- **Limits** (`halftone-types/src/limits.rs`) are sized for 2 GB peak on an
  8 GB Mac; check new allocations against them.
- Keep files and functions small; no new crate without a reason recorded in the TODO.

## Docs go with the code

Every change, including side fixes, is recorded in the docs:

- `docs/TODO.md`: tick or add the item; record decisions in the decision table
  with the date.
- CLI options or exit codes → `docs/CLI.md` (and `--help` text in `args.rs`).
- Config fields → `docs/configuration.md` + regenerated schema.
- Crate structure or pipeline → `docs/architecture.md`.

## Git

- Work on `main` unless Kyo names a branch; never create branches or worktrees
  without asking.
- Commit only when Kyo asks; never push unasked.
- Don't commit photos or generated output (see `.gitignore`).

## Open items that need Kyo

Print tests (Experiment E: LPI, angle, minimum dot, shadow fill-in, printer
margin) and the Affinity Designer / Inkscape import check. Profile defaults in
`crates/halftone/src/profile.rs` are provisional until then; don't "tune" them
without print results.
