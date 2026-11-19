# Halftone Algos

## Initial Algorithm Research and Rust Implementation Recommendations

**Companion document to:** `Halftone.md`\
**Document status:** Research draft 0.1\
**Research date:** 2026-11-16\
**Scope:** Initial investigation of halftoning mathematics, print
constraints, image processing, Rust dependencies, geometry union, SVG
construction, and validation.

------------------------------------------------------------------------

## 1. Executive Summary

The recommended initial algorithm is a **single-channel,
amplitude-modulated (AM), clustered-dot screen**:

1.  Normalize the source image and convert it to a documented luminance
    representation.
2.  Fit the image inside the requested physical output canvas.
3.  Define a regular Cartesian screen in physical units using the
    selected LPI.
4.  Rotate the screen coordinate system by the configured angle.
5.  Sample the source image at each screen-cell center.
6.  Convert the sampled tone into a target ink-area coverage.
7.  Convert coverage into a circle diameter using a calibrated
    spot-growth function.
8.  Generate circle geometry in physical coordinates.
9.  Clip the artwork to the output canvas.
10. Union the marks into a compound filled region.
11. Serialize and validate a transparent-background SVG.

This is the most direct match for the product's current scope: regular
grid, variable-size circular marks, single ink, positive artwork, and
print-oriented physical dimensions. AM screening is the established
family of halftoning methods in which dots are arranged on a regular
lattice and vary in size with tone. The classic digital implementation
uses a periodic screen/threshold pattern; it is commonly called
clustered-dot or AM halftoning. [1, 2]

### Initial recommendations

| Area | Recommendation | Confidence |
| --- | --- | --- |
| Halftone family | AM / clustered-dot | High |
| Grid | Regular Cartesian lattice | High |
| Initial mark | Circle | High |
| Tone model | Target ink-area coverage, then spot growth | High |
| Sampling | Cell-center bilinear sample initially; compare with cell-area average | Medium |
| Physical units | Canonical millimeters or micrometers internally | High |
| 110-mesh starting LPI | 22 LPI conservative starting preset; expose 22–27.5 as an advisory range | Medium |
| Screen angle | Configurable; 22.5° is a candidate preset, not a universal rule | Medium-low |
| Image processing | `image` plus optional ICC conversion via `lcms2` | Medium-high |
| Parallelism | Rayon for independent per-cell work | High |
| Boolean union | Benchmark `i_overlay` against `clipper2` | Medium |
| SVG | One compound path with a single fill | High |

The LPI and angle defaults must be validated through real screen tests.
Mesh count alone is insufficient to guarantee a printable result.

------------------------------------------------------------------------

## 2. Halftone Algorithm Families

### 2.1 AM / clustered-dot screening

AM screening uses a regular lattice of halftone cells. Each cell
contains a mark whose size changes with the source tone. Darker image
values produce larger ink dots; lighter values produce smaller dots.
This is the familiar photographic halftone used in traditional print
reproduction. [1, 2]

**Why it fits this project**

-   It directly implements the desired regular X/Y grid.
-   LPI maps naturally to physical cell spacing.
-   Dot size is a controllable geometric variable.
-   The output can be generated as vector circles.
-   A regular grid is deterministic and easy to test.
-   It provides a useful base for later dot shapes and grid strategies.

**Limitations**

-   Regular patterns can interact with screen mesh and source-image
    structure to create visible moiré.
-   Small highlight dots may fail to expose or print.
-   Large shadow dots can merge, reducing tonal separation.
-   A fixed grid does not preserve every local photographic detail.

Those limitations are acceptable for the stated goal, which prioritizes
robust print artwork over maximum photographic detail.

### 2.2 Stochastic / FM screening

Frequency-modulated (FM) screening represents tone primarily through the
distribution or density of marks rather than changing the size of marks
on a regular lattice. Blue-noise distributions are a common way to avoid
obvious periodic structure. Research literature contrasts AM's regular,
variable-size clusters with stochastic approaches. [3]

**Recommendation:** Do not implement in the first release. Keep the
grid-generation interface sufficiently modular that a future stochastic
placement strategy can feed the same tonal and geometry pipeline.

### 2.3 Adaptive and other geometric grids

Adaptive placement, hexagonal grids, and isometric grids are future
extensions. They should not influence the first release's core data
model more than necessary.

The first release should avoid designing a universal pattern framework
before there are multiple real implementations. A small grid strategy
abstraction is sufficient.

------------------------------------------------------------------------

## 3. Recommended AM Pipeline

### 3.1 Stage A — Normalize the source image

Produce a normalized image representation with:

-   Applied orientation metadata.
-   Explicit alpha handling.
-   Known color-space assumptions.
-   A luminance channel or a representation from which luminance can be
    computed.
-   Validated dimensions and safe allocation limits.

Avoid tying the core algorithm to a decoder-specific pixel type.

### 3.2 Stage B — Establish physical layout

Convert the requested output dimensions into a canonical physical
coordinate system.

Recommended internal convention:

-   Millimeters for physical geometry, or
-   Integer micrometers for robust quantized geometry where appropriate.

Use floating-point values for image sampling and tonal calculations.
Convert to the geometry engine's chosen representation at a clearly
defined boundary.

The output SVG can use inches or centimeters in its `width` and `height`
attributes, while its `viewBox` uses the internal coordinate scale.

### 3.3 Stage C — Fit the source image

The default `contain` behavior preserves the complete photograph and
aspect ratio. It should calculate the physical image rectangle inside
the output canvas, then sample only within that rectangle.

The area outside the placed image is not ink. It should not accidentally
be sampled as black.

If the user chooses crop mode, crop coordinates and anchor must be
resolved before grid sampling.

### 3.4 Stage D — Define the halftone lattice

For a nominal LPI value $L$, the unrotated cell pitch is:

$$
p = \frac{25.4}{L}\ \text{mm}
$$

For example:

| LPI | Cell pitch |
| ---: | ---: |
| 22 | 1.1545 mm |
| 24 | 1.0583 mm |
| 27.5 | 0.9236 mm |
| 35 | 0.7257 mm |
| 45 | 0.5644 mm |

A screen rotation should rotate the lattice in physical output space.
For grid coordinates $(i, j)$, an unrotated center can be represented as:

$$
\mathbf{q}_{ij} =
\begin{bmatrix}
(i + \tfrac12)\,p_x \\
(j + \tfrac12)\,p_y
\end{bmatrix}
$$

Apply a rotation matrix around a defined screen origin:

$$
R(\theta) =
\begin{bmatrix}
\cos\theta & -\sin\theta \\
\sin\theta & \cos\theta
\end{bmatrix}
$$

$$
\mathbf{x}_{ij} = \mathbf{o} + R(\theta)\,\mathbf{q}_{ij}
$$

The inverse transform maps a physical point back into screen coordinates
for sampling and grid indexing.

**Important implementation detail:** Rotating a rectangular array of
precomputed circles around the canvas center is not equivalent to
defining an infinite rotated lattice and selecting the cells that
intersect the output canvas. The latter is the more reliable model.
Calculate a conservative rotated bounding region, enumerate the required
lattice indices, and clip final geometry to the canvas.

The implementation must document whether LPI is measured along the
lattice axes. For a square grid this means equal pitch along the two
grid basis vectors. Do not derive row and column counts by simply
multiplying the unrotated canvas width and height by LPI after rotation.

### 3.5 Stage E — Sample the image

Two sampling strategies deserve comparison.

**Cell-center sample**

-   Transform the cell center into source-image coordinates.
-   Use bilinear interpolation.
-   Low cost and simple behavior.
-   Can miss small features or noise near cell boundaries.

**Cell-area average**

-   Estimate average luminance over the cell footprint.
-   Better represents the cell's overall tone.
-   More computationally expensive, especially for rotated cells.
-   Can be implemented with supersampling or an integral-image strategy
    for axis-aligned cases.

**Initial recommendation:** Implement cell-center bilinear sampling
first, but design a sampling interface so area averaging can be added
without changing tonal mapping or geometry generation. Compare both on
photographic fixtures. Since fine detail is not the primary goal, center
sampling is a reasonable first baseline.

For a rotated grid, the source sampling point is obtained by applying
the inverse output-to-screen transform, then mapping through the fitted
image rectangle into pixel coordinates.

### 3.6 Stage F — Convert luminance to target ink coverage

Use a normalized luminance $Y$, where:

-   $Y = 0$ means black.
-   $Y = 1$ means white.

The simplest positive-artwork mapping is:

$$
C = 1 - Y
$$

where $C$ is the target ink-area coverage.

Apply user-adjustable tonal response before calculating coverage. A
simple configurable model could be:

$$
C' = \operatorname{clamp}\left( \operatorname{clamp}(C + b,\, 0,\, 1)^{\gamma},\, 0,\, 1 \right)
$$

where $b$ is a tonal offset and $\gamma$ controls the response.
The exact order and interpretation of contrast, black point, and white
point should be specified and tested rather than exposing several
overlapping controls with ambiguous effects.

The default mapping must be monotonic: increasing darkness must never
result in a smaller mark.

For inverted tonal mapping, use the complementary coverage mapping,
while keeping the output a transparent positive. Define inversion
precisely in terms of source tone and target ink coverage so that users
do not confuse it with a black-background negative.

------------------------------------------------------------------------

## 4. Circle Diameter Is Not a Simple Linear Tone Mapping

This is one of the most important algorithmic decisions.

If the target ink coverage of one isolated circular mark is $C$, and the
cell area is $p^2$, then equating circle area to target coverage gives:

$$
C\,p^2 = \pi \left(\frac{d}{2}\right)^2
$$

Therefore:

$$
d = p\sqrt{\frac{4C}{\pi}}
$$

This is a useful first approximation for isolated circles. It is **not
sufficient for the complete tonal range of a periodic halftone screen**.

### 4.1 Why the simple formula breaks down

For a square lattice:

-   When $d < p$, neighboring circles are separate.
-   At $d = p$, adjacent circles touch at their edges.
-   When $d > p$, circles overlap and the union changes the actual ink
    coverage.
-   At $d = \sqrt{2}\,p$, the circle radius reaches the cell corner;
    the periodic circle union covers the entire plane.

The simple area formula assumes that circles do not overlap. Once they
overlap, adding the areas of individual circles double-counts the
overlap. If the application wants a predictable tonal response, it
should map the requested coverage to the **actual covered area of the
periodic circle lattice**.

### 4.2 Recommended solution: calibrated periodic spot-growth function

Create a function:

``` text
coverage_to_diameter(target_coverage, cell_pitch) -> diameter
```

For a square lattice of equal circles, define the actual coverage as the
area of the union of all periodic circles inside one fundamental square
cell, divided by the cell area.

Build a monotonic lookup table:

1.  Choose a normalized diameter range from zero to $\sqrt{2}$
    cell pitches.
2.  For each diameter, calculate the union coverage in one periodic
    cell.
3.  Use a sufficiently accurate numerical method or a trusted geometry
    implementation.
4.  Store `(coverage, normalized_diameter)` samples.
5.  Invert the table using interpolation to map target coverage to
    diameter.
6.  Test monotonicity, endpoints, and approximation error.

A more direct analytic or numerically integrated implementation may be
preferable after benchmarking. The lookup table is a practical starting
point because it is small, deterministic, and fast at generation time.

**Do not use the isolated-circle formula beyond the non-overlapping
range without accounting for overlap.**

### 4.3 Calibration and print gain

The geometric coverage is not identical to printed coverage. Screen
exposure, stencil behavior, ink, substrate, pressure, and dot gain all
affect the final print.

The application should distinguish:

-   `geometric_coverage`: coverage represented by the SVG geometry.
-   `target_coverage`: coverage requested by tonal mapping.
-   `print_compensation`: optional user-provided correction curve or
    preset.

Do not claim that a mathematical SVG coverage model predicts the final
printed tonal value without a physical calibration workflow.

A future calibration feature could use a printed tonal ramp and a
measured or visually selected correction curve. For the initial release,
keep compensation optional and avoid inventing universal dot-gain
values.

------------------------------------------------------------------------

## 5. Luminance and Color Management

### 5.1 Decode first; interpret color second

The `image` crate is a strong initial candidate for decoding common
formats. Its documentation describes native Rust decoding/encoding and
generic image buffers, with common formats supported by default
features. It is designed as a relatively safe, stable base that can be
supplemented by specialized libraries. [7]

However, image decoding and color management are separate
responsibilities. The application should not assume that decoded RGB
byte values are already in a common color space.

### 5.2 Recommended color path

A robust initial color path is:

1.  Decode the source file.
2.  Obtain the embedded ICC profile if available.
3.  Convert profiled input to a known working RGB space, preferably
    linear-light sRGB for the luminance stage.
4.  If no profile exists, apply a documented default assumption,
    normally sRGB for common consumer images.
5.  Convert nonlinear sRGB channel values to linear-light values.
6.  Calculate relative luminance.
7.  Apply tonal controls.

For linear-light sRGB / Rec. 709 primaries, the common relative
luminance calculation is:

$$
Y = 0.2126R + 0.7152G + 0.0722B
$$

The RGB values in this formula must be linear-light values, not the
original gamma-encoded sRGB channel values.

### 5.3 ICC library candidate: `lcms2`

The Rust `lcms2` crate is a wrapper around Little CMS and exposes
profile and transform abstractions. Its documentation describes ICC
profile handling and conversion through Little CMS. [8]

**Recommendation:** Evaluate `lcms2` as the color-management backend. It
is a mature C library with Rust bindings, but introduces a native
dependency/build consideration. Confirm macOS, Windows, and Linux build
behavior in the intended distribution environment.

Do not make the image crate's decoder the owner of color-management
policy. Keep ICC extraction, profile selection, and transforms behind
the `halftone-image` crate's own interface.

### 5.4 `imageproc` caution

`imageproc` is a useful Rust image-processing library and offers
Rayon-backed parallel variants for some operations. Its documentation
explicitly notes that its functions assume linear color-space pixel
values in relevant contexts and recommends benchmarking parallel
variants for the workload. [9]

The Halftone application does not need a large general-purpose
image-processing dependency simply to compute luminance and sample a
grid. Use small, explicit routines where they are clearer and faster.
Adopt `imageproc` only where a required operation justifies it.

------------------------------------------------------------------------

## 6. Mesh Count, LPI, and Physical Constraints

### 6.1 Mesh-to-LPI rule of thumb

A screen-printing support guide recommends choosing mesh count at
approximately 4–5 times the halftone LPI. It gives the example of 45
LPI requiring roughly 180–225 mesh, with process quality influencing
where within that range to work. [4]

For a 110-mesh screen:

$$
\mathrm{LPI}_{\text{conservative}} = \frac{110}{5} = 22
$$

$$
\mathrm{LPI}_{\text{less conservative}} = \frac{110}{4} = 27.5
$$

This supports a conservative initial 22 LPI preset, with 22–27.5 LPI
presented as a starting range rather than a guarantee.

### 6.2 Important caveat: mesh count is not enough

The mesh-to-LPI ratio is a heuristic. The ability to hold a halftone
depends on factors including:

-   Thread diameter.
-   Mesh opening and open area.
-   Mesh material and weave.
-   Screen tension.
-   Emulsion and stencil thickness.
-   Exposure quality.
-   Ink characteristics.
-   Substrate.
-   Squeegee and press conditions.

A Murakami technical newsletter specifically discusses mesh type, thread
thickness, halftone angle, and the interaction of the screen with fine
tonal values. It illustrates why a mesh-count-only rule is incomplete.
[6]

The CLI should therefore label its recommendation as an estimate. The
JSON should record the formula and assumptions used.

### 6.3 Proposed recommendation model

For a first version:

``` text
recommended_lpi = mesh_count / selected_ratio
```

Where:

-   `selected_ratio` defaults to 5.0 for a conservative recommendation.
-   A user may select 4.5 or 4.0 as a less conservative setting.
-   Explicit `--lpi` overrides the recommendation.
-   The app emits a warning when explicit LPI is outside its selected
    advisory range.

This model is intentionally transparent. Do not hide the recommendation
behind an opaque formula.

### 6.4 Mesh units and naming

Use a clear configuration field such as:

``` json
{
  "mesh": {
    "count_per_inch": 110,
    "thread_diameter_um": null,
    "mesh_opening_um": null
  }
}
```

Mesh-count terminology can vary by market and supplier. The
implementation should document that `count_per_inch` means the linear
thread count used by the selected convention, not threads per square
inch.

If thread diameter or opening is unknown, do not invent it.

### 6.5 Dot size and highlight survival

At 22 LPI, cell pitch is approximately 1.155 mm. A 5% target geometric
coverage, in the non-overlapping circle region, gives an approximate
circle diameter:

$$
d = 1.155\sqrt{\frac{4(0.05)}{\pi}} \approx 0.292\ \text{mm}
$$

At 10% coverage, the diameter is approximately 0.412 mm.

These figures are geometric examples, not guaranteed minimum printable
dots. The smallest highlights may disappear depending on the screen and
process. The application should let the user impose a minimum feature
size or tonal cutoff and should report when the requested tonal range
implies very small geometry.

### 6.6 Screen angle

A 22.5° angle is commonly suggested in some single-color screen-printing
workflows, but sources differ and practical angle selection is affected
by mesh weave and thread construction. Multi-channel color separation
conventions are not directly transferable to a single-color halftone.

The Murakami technical material emphasizes evaluating halftone angle in
relation to mesh and thread type. [6]

**Recommendation:**

-   Expose `--angle` in degrees.
-   Use a configurable default profile.
-   Treat 22.5° as a candidate starting preset, not a universal
    technical truth.
-   Keep the screen origin/phase configurable in the algorithm, even if
    the CLI does not initially expose it.
-   Provide a test chart so users can compare candidate angles on their
    actual screen.

A later research task should evaluate a small angle sweep (for example
15°, 22.5°, 30°, and 45°) for representative mesh types, but the final
selection should come from test exposure and print results.

------------------------------------------------------------------------

## 7. Rust Crate Evaluation

The following is a shortlist for prototype evaluation, not a locked
dependency manifest. Versions change; pin versions only when the
workspace is created and tested.

### 7.1 Image decoding: `image`

**Candidate:** [`image`](https://docs.rs/image/latest/image/)

Why it is promising:

-   Common image formats.
-   Native Rust implementations.
-   Unified image reader and image-buffer APIs.
-   Support for feature selection to control build size.
-   A reasonable abstraction for the `halftone-image` crate.

The current documentation describes `image` 0.25.x and support for
common decoding and encoding operations. [7]

**Recommendation:** Start with `image`, explicitly enable only required
format features after confirming the exact feature names for the chosen
version. Test JPEG, PNG, and TIFF fixtures, including higher bit depth
and orientation behavior.

Do not assume every decoder exposes ICC metadata in the same way. Verify
this for each format.

### 7.2 Color management: `lcms2`

**Candidate:** [`lcms2`](https://docs.rs/lcms2/latest/lcms2/)

Why it is promising:

-   Rust wrapper around Little CMS.
-   ICC profiles and transforms.
-   Mature underlying color-management engine.
-   MIT-licensed Rust wrapper according to current package metadata.
    [8]

**Trade-off:** Native C dependency and build/distribution complexity.

**Recommendation:** Prototype early on macOS, Windows, and Linux. If
native dependency management is problematic, evaluate pure-Rust
alternatives, but do not sacrifice correct profile interpretation
without documenting the limitation.

### 7.3 Parallel processing: `rayon`

**Candidate:** [`rayon`](https://docs.rs/rayon/latest/rayon/)

Rayon provides data-parallel iterators and work-stealing parallelism,
with safe data-race-free execution. [10]

Good uses in this project:

-   Parallel luminance conversion.
-   Parallel independent cell sampling.
-   Parallel tone-to-diameter mapping.
-   Parallel primitive construction into indexed output buffers.

Avoid parallelizing tiny loops where scheduling overhead exceeds the
work. Preserve deterministic ordering by collecting results in a stable
indexed sequence.

**Recommendation:** Use Rayon behind internal processing functions, with
a sequential path retained for tests and small workloads.

### 7.4 Boolean geometry: `i_overlay`

**Candidate:**
[`i_overlay`](https://docs.rs/i_overlay/latest/i_overlay/)

The crate documents Boolean union, intersection, difference, XOR,
support for holes and multiple contours, fill rules, simplification, and
integer and floating-point APIs. It is described as the overlay engine
used by the Rust `geo` ecosystem. [11]

Why it deserves first evaluation:

-   Rust-native implementation.
-   Polygon Boolean operations.
-   Multiple numeric APIs.
-   Fill-rule support.
-   Simplification and degenerate-geometry handling.
-   Optional multithreading is documented in the crate materials. [11]

**Potential concern:** Circles must be represented as polygons if the
selected API operates on polygonal inputs. Polygon approximation
tolerance must be controlled in physical units.

**Recommendation:** Make `i_overlay` the first Rust-native candidate in
the geometry prototype. Do not commit to it until it passes the
project's own adversarial geometry tests and benchmarks.

### 7.5 Boolean geometry alternative: `clipper2`

**Candidate:** [`clipper2`](https://docs.rs/clipper2/latest/clipper2/)

The Rust crate exposes Clipper2 operations including union,
intersection, difference, offsetting, and simplification. Its current
binding uses `clipper2c-sys`, a wrapper around the C++ Clipper2
implementation. [12]

Why compare it:

-   Mature Clipper family.
-   Well-known polygon clipping and offset operations.
-   Union of path collections.
-   Simplification and offsetting may also help linocut constraints.

Trade-offs:

-   C++ native dependency through FFI.
-   Build and packaging considerations.
-   Need to understand integer scaling and precision behavior.

**Recommendation:** Benchmark `clipper2` against `i_overlay` using
identical workloads. A native Rust dependency is attractive, but
geometry correctness and throughput matter more than language purity.

### 7.6 CLI and configuration

Recommended candidates:

-   `clap` for command parsing.
-   `serde` and `serde_json` for configuration.
-   `thiserror` for typed crate errors.
-   `anyhow` only at the CLI/application boundary, if useful; `miette`
    is an alternative when richer diagnostics are wanted.
-   `schemars` to generate the configuration JSON Schema from the same
    Rust types that `serde` uses, so schema and code cannot drift.
-   `serde_path_to_error` so configuration errors name the failing
    field path (for example `halftone.minimum_diameter`), supporting the
    actionable diagnostics required by the specification.
-   `tracing` and `tracing-subscriber` with one span per pipeline stage;
    `--verbose` then reports stage timings without bespoke timing code.
-   `indicatif` for progress reporting; its Rayon integration
    (`ParallelProgressIterator`) covers the parallel stages.
-   `ctrlc` with an `AtomicBool` cancellation flag checked between
    stages, so an interrupted run never leaves a partial SVG.
-   `tempfile` (`NamedTempFile::persist`) for write-then-rename output.
-   `blake3` for the optional source-file hash; its `rayon` feature
    hashes large files in parallel.
-   `mimalloc` as the CLI's global allocator. Geometry generation makes
    many small allocations across Rayon worker threads, a workload where
    an alternative allocator often helps. Adopt only if benchmarks show
    a gain.

These are conventional Rust ecosystem choices, but verify current
versions, feature configuration, and MSRV when creating the workspace.

### 7.7 Testing and benchmarks

Recommended candidates:

-   `proptest` for property-based tests.
-   `criterion` for repeatable microbenchmarks (`divan` is a lighter
    alternative).
-   `insta` only where snapshot testing is useful.
-   `approx` for floating-point assertions with explicit tolerances.
-   `dhat` for peak-heap measurement per pipeline stage.
-   `resvg` / `usvg` / `tiny-skia` to rasterize generated SVGs in tests.
    This enables filled-area-versus-predicted-coverage assertions
    (Experiment C), rasterized visual regression (§11.5), and fill-rule
    and hole checks (§8.7) without external tools.
-   `cargo-nextest` and `cargo-deny` (licenses, advisories) in CI.
-   Standard Rust unit and integration tests.

Do not rely solely on snapshots of serialized SVG. Test geometry
properties and rendered results as well.

### 7.8 Further candidates

The candidates in this section come from general ecosystem knowledge,
not from documentation reviewed for this draft. Confirm maintenance,
licensing, and fit during Phase 0 before adopting any of them. Items
marked *(verify)* are newer or less certain.

**Image pipeline**

-   `fast_image_resize` — SIMD-accelerated resampling. This is probably
    the largest single performance and memory lever: a 50 MP photograph
    printed at 8 × 10 in and 22 LPI needs only about 40,000 screen
    cells. Downsample once to a luminance plane at a few pixels per
    cell, and no later stage touches full-resolution floating-point
    data. The chosen working resolution must keep sampling error within
    a tested tolerance.
-   `moxcms` — pure-Rust color management, a possible alternative to
    `lcms2` that avoids the native C dependency *(verify)*. Compare in
    Experiment A.
-   A 256-entry lookup table for 8-bit sRGB-to-linear conversion is
    faster than a general color library for this step. `palette` is
    useful as a reference implementation in tests.

**Geometry**

-   `geo-types` — **chosen as the shared geometry type** (decision D33,
    2026-11-18), including in public APIs, as a deliberate exception to
    spec §14.1. Integer micrometre coordinates (`MultiPolygon<i32>`), y
    down. The `geo` crate's Boolean operations use `i_overlay`, the engine
    selected in Phase 0, so the two interoperate directly.
-   `rstar` — R-tree spatial index for minimum-gap and nearest-feature
    checks (§10.2).
-   `kurbo` — curve primitives and flattening. Used locally for curves
    such as registration marks; not the interchange type (D33). Relevant
    again if curve-aware circles are pursued (§8.2 Option B). `linesweeper`
    provides curve Boolean operations on top of it *(verify)*.

**SVG output**

-   Direct serialization with `std::fmt::Write` is sufficient for one
    compound `<path>`; a general SVG-writing crate adds little.
-   `ryu` — fast float formatting. Round to the chosen output precision
    before formatting to keep output compact and deterministic.
-   `roxmltree` — lightweight well-formedness check in the output
    validator.

**Deliberately not recommended initially**

-   `tokio` or other async runtimes: every stage is CPU-bound, so async
    adds complexity without throughput.
-   Channel-based stage pipelines (`crossbeam-channel`): stages are
    sequential barriers, so overlapping them buys little. Rayon covers
    the threading.
-   Explicit SIMD crates (`wide`, `pulp`): row-oriented loops over flat
    buffers usually auto-vectorize. Revisit only if profiling points
    there.
-   `uom`: newtype units such as `Millimeters(f64)` keep units explicit
    with less type complexity.
-   Arena allocators (`bumpalo`): consider only if `dhat` shows
    allocation pressure that `mimalloc` and buffer reuse do not solve.

------------------------------------------------------------------------

## 8. Geometry Representation and Union Strategy

### 8.1 Keep geometry separate from SVG

The geometry crate should produce a library-independent representation
of the filled artwork. The SVG crate should serialize that
representation.

This allows the geometry engine to change without rewriting the SVG
serializer.

### 8.2 Circle representation

There are two viable approaches.

**Option A — Polygonal circles**

Approximate each circle with a polygon, then union the polygons.

Advantages:

-   Directly compatible with polygon Boolean engines.
-   Straightforward geometry validation.
-   One consistent representation through union.

Disadvantages:

-   More vertices.
-   Approximation error.
-   More work for the Boolean engine.

**Option B — Curve-aware Boolean operations**

Retain circles or cubic Bézier paths as curves through a curve-capable
Boolean engine.

Advantages:

-   Potentially fewer vertices.
-   Better preservation of smooth circular outlines.
-   Smaller SVG path output.

Disadvantages:

-   More specialized geometry engine.
-   Must evaluate robustness and maintenance.
-   Curve intersections and coincident boundaries are complex.

**Recommendation:** Prototype polygonal circles first using an explicit
maximum radial error in physical units. Evaluate a curve-aware option if
polygon union becomes a major performance or file-size bottleneck.

### 8.3 Polygon approximation tolerance

Do not choose a fixed number of vertices per circle without considering
radius and output scale.

For a regular n-sided polygon approximating a circle of radius $r$, the
maximum radial sagitta error is:

$$
e = r\left(1 - \cos\frac{\pi}{n}\right)
$$

Rearranging gives a minimum segment count:

$$
n \geq \frac{\pi}{\arccos(1 - e/r)}
$$

The implementation should:

-   Define an allowed physical error, such as a configurable fraction of
    a millimeter.
-   Compute the required segment count from radius and tolerance.
-   Enforce a sensible minimum and maximum segment count.
-   Use deterministic vertex placement.
-   Test the resulting maximum deviation.

The final tolerance should be selected through print-scale tests and SVG
file-size/performance benchmarks.

### 8.4 Union strategy

A naive all-at-once union may become expensive for thousands or hundreds
of thousands of marks.

Benchmark at least:

1.  Union all circles in one operation.
2.  Batch unions by spatial tile, then union tile results.
3.  Use a spatial index or bounding-box filtering to reduce irrelevant
    intersection work, if supported by the chosen engine.
4.  Retain non-overlapping components without unnecessary pairwise
    checks where the engine supports efficient union of a collection.

Batching must not introduce seams, gaps, or inconsistent topology at
tile boundaries. A tile-based strategy must include sufficient overlap
or a correct shared-boundary treatment.

### 8.5 Integer versus floating-point geometry

Integer geometry can provide predictable quantization and robust
topology, but requires choosing a scale. Floating-point geometry avoids
explicit quantization but still needs robust handling of near-coincident
boundaries.

Prototype both modes if supported by the candidate engine.

If integer coordinates are used:

-   Choose a documented scale such as geometry units per millimeter.
-   Ensure the maximum output dimensions cannot overflow the chosen
    integer type.
-   Quantize only once at the geometry boundary.
-   Test the maximum quantization error.
-   Keep physical dimensions in the JSON independent of the geometry
    engine's internal integer scale.

### 8.6 Clipping

Clip the final union to the output canvas, or clip primitives before
union if benchmarking shows that is more efficient and produces
equivalent results.

Test marks centered partly outside the canvas, especially on rotated
grids. The SVG should not contain unintended geometry outside its
viewBox.

### 8.7 Compound path semantics

A single SVG `<path>` may contain multiple disconnected subpaths. This
is still one path element and is appropriate for a single-ink positive.

The serializer must preserve holes using a compatible fill rule. Test
the actual result in at least two independent SVG renderers or vector
applications.

------------------------------------------------------------------------

## 9. Sampling and Grid Geometry Details

### 9.1 Stable screen phase

A periodic grid has both an angle and a phase/origin. Changing the
origin shifts the dot lattice relative to the photograph, potentially
changing how features are represented.

The analysis JSON should eventually record a stable screen origin or
phase. The initial CLI can keep the default origin fixed, but it must
not be an undocumented implementation accident.

### 9.2 Source coordinate mapping

The processing pipeline should have explicit transforms:

-   Source pixel coordinates.
-   Fitted image coordinates.
-   Physical output coordinates.
-   Screen lattice coordinates.

Avoid mixing pixel dimensions, inches, and millimeters in the same
untyped calculation.

A `PhysicalPoint`, `PixelPoint`, or equivalent domain types can help
prevent accidental coordinate-space mixing.

### 9.3 Edge behavior

For cells whose centers fall outside the fitted image rectangle:

-   Do not sample arbitrary out-of-bounds pixels.
-   Do not create marks unless the selected edge policy explicitly
    requires them.
-   Keep the canvas transparent outside the fitted photograph.

The edge policy must be deterministic and tested for portrait,
landscape, and square images.

------------------------------------------------------------------------

## 10. Print-Oriented Feature Constraints

### 10.1 Minimum feature width

The application should support a minimum ink-feature width in physical
units. This may be applied as:

-   A tonal cutoff that suppresses marks below a threshold.
-   A geometry cleanup operation.
-   A validation warning.
-   A future compensation operation.

These approaches are not equivalent. The initial implementation should
prefer a clear, predictable suppression rule over aggressive geometry
repair.

### 10.2 Minimum gap

The minimum gap is the minimum clear distance between separate ink
regions. It is distinct from minimum ink width.

A gap constraint can be enforced by:

-   Warning when neighboring marks are too close.
-   Adjusting mark diameter.
-   Applying geometry offsets/erosion.
-   Suppressing or merging marks.

Do not silently modify geometry in ways that significantly change tone.
Any corrective operation should be recorded in diagnostics and, where
applicable, in the output metadata or generation report.

### 10.3 Linocut profile

A linocut profile may use:

-   Coarser grid frequency.
-   Larger minimum marks.
-   Larger minimum gaps.
-   More aggressive tiny-mark suppression.
-   More conservative geometry simplification.

The exact values depend on carving material, tool, scale, and print
method. Do not hard-code a universal "linocut-safe" feature width
without user-selectable settings and physical tests.

### 10.4 Geometry complexity limit

The application should estimate:

-   Number of generated marks.
-   Total polygon vertices.
-   Estimated union input size.
-   Peak memory risk, where estimable.

Warn or stop before attempting an operation likely to exhaust memory.
Allow explicit user override of configurable safety limits.

------------------------------------------------------------------------

## 11. Testing the Algorithms

### 11.1 Tonal mapping tests

Use synthetic grayscale ramps and known values.

Required tests:

-   Black maps to maximum intended coverage.
-   White maps to zero ink coverage.
-   Midtones map monotonically.
-   Inversion reverses the mapping.
-   Gamma changes the curve in the documented direction.
-   Diameter remains within configured bounds.
-   Coverage-to-diameter and diameter-to-coverage agree within
    tolerance.

### 11.2 Periodic circle coverage tests

This deserves its own test module.

Test target coverage at values such as:

``` text
0%, 1%, 2%, 5%, 10%, 25%, 50%, 75%, 90%, 99%, 100%
```

Verify:

-   Diameter is monotonic.
-   Coverage is monotonic.
-   The zero and full-coverage endpoints are correct.
-   The lookup interpolation error stays within a defined tolerance.
-   The implementation handles the circle-overlap transition near one
    cell pitch.

### 11.3 Grid tests

Use known physical dimensions and LPI values.

Test:

-   Zero-degree grid.
-   Candidate angles.
-   Negative angles and normalization.
-   Portrait and landscape canvases.
-   Cells just touching canvas boundaries.
-   Stable phase.
-   Correct row/column enumeration for rotated bounds.

### 11.4 Geometry tests

Construct synthetic cases:

-   Two disjoint circles.
-   Two tangent circles.
-   Two overlapping circles.
-   Many overlapping circles.
-   Nested contours.
-   A ring with a hole.
-   Shapes clipped at the canvas edge.
-   Near-coincident boundaries.
-   Very small and very large radii.

Validate area, bounds, topology, hole behavior, and SVG fill results.

### 11.5 Visual regression tests

Keep a small fixture set:

-   Smooth grayscale gradient.
-   Hard black-and-white edge.
-   Portrait photograph.
-   Landscape photograph.
-   High-frequency texture.
-   Mostly dark image.
-   Mostly light image.
-   Color image with saturated colors.

Render generated SVGs to raster previews in test tooling and compare
meaningful properties. A visual review of representative outputs remains
necessary; numerical tests alone cannot establish print quality.

### 11.6 Physical test chart

Generate a dedicated screen-print test chart containing:

-   Tonal patches.
-   Dot-size ramps.
-   Several LPI settings.
-   Several angles.
-   Minimum-feature examples.
-   Minimum-gap examples.
-   A geometry scale reference.

For the 110-mesh starting workflow, test 22, 24, and 27.5 LPI rather
than declaring one value universally correct. Record the actual screen
mesh construction, emulsion, exposure, ink, substrate, and print
outcome.

------------------------------------------------------------------------

## 12. Performance Plan

### 12.1 Likely cost centers

The likely performance profile is:

1.  Image decoding: potentially significant for large TIFF files.
2.  Color conversion and luminance: linear work over source pixels.
3.  Grid sampling and tone mapping: linear work over generated cells.
4.  Primitive generation: linear work over marks and vertices.
5.  Boolean union: potentially dominant and nonlinear with geometry
    complexity.
6.  SVG serialization: linear in final path complexity.

Do not optimize the image loop while ignoring a geometry union that
consumes most of the runtime.

### 12.2 Parallel work

Use Rayon for independent per-pixel or per-cell work where benchmarks
justify it. The Rayon documentation describes safe parallel iterators
and work-stealing execution. [10]

Recommended first parallel targets:

-   Normalize image rows or tiles.
-   Convert pixels to luminance.
-   Evaluate grid cells.
-   Generate primitive vertex buffers.

Avoid shared mutable geometry collections. Prefer indexed parallel maps
or per-worker buffers followed by deterministic concatenation.

Recommended patterns:

1.  **Pixel stages** (normalization, luminance): process a flat buffer
    with `par_chunks_exact_mut(width)`, one row per task. No locks are
    needed, and the inner loops can be vectorized by the compiler.
2.  **Per-cell stages** (sampling, tonal mapping, primitive
    generation): iterate rows in parallel, for example
    `(0..rows).into_par_iter().flat_map_iter(|r| cells_in_row(r))`, and
    `collect()` into a `Vec`. Rayon's indexed collection preserves
    order, so output is deterministic.
3.  **Boolean union** (§8.4): assign each primitive to a spatial tile by
    its center, without clipping, so tile borders need no seam
    handling. Union each tile in parallel, then merge tile results
    pairwise with `rayon::join` in a fixed tree order. Each merge
    handles fewer edges than one global union, and the fixed order
    keeps output stable. Benchmark against the engine's own
    all-at-once union and any built-in multithreading.
4.  **Determinism:** do not let a Rayon `reduce` or `sum` over
    floating-point values influence geometry, because grouping varies
    between runs. Collect indexed results, then reduce sequentially or
    in a fixed tree.
5.  **Thread control:** expose `--threads` through
    `rayon::ThreadPoolBuilder`, with `RAYON_NUM_THREADS` as the
    fallback. Use a single thread in tests that assert exact output.
6.  **No async or stage pipelining:** see §7.8.

### 12.3 Memory layout

Possible internal representations:

-   `Vec<f32>` luminance plane.
-   Compact per-cell sample/diameter arrays.
-   Reusable geometry buffers.
-   Streaming SVG output only after the final geometry is available.

A full luminance plane costs approximately four bytes per pixel when
stored as `f32`. A 24-megapixel image therefore requires about 96 MB for
that plane alone, excluding decoded source pixels and geometry. Consider
`f32` versus `u16`/`f16` only after accuracy and performance
measurements.

Downsampling to an analysis resolution tied to the cell pitch (§7.8,
`fast_image_resize`) shrinks this plane by orders of magnitude for
typical print sizes and should be evaluated first. Reuse buffers with
`Vec::with_capacity`, and use `smallvec` for per-circle vertex lists
only if profiling shows allocator pressure.

### 12.4 Benchmark matrix

Benchmark at least:

| Case | Image size | Approx. grid density |
| --- | ---: | --- |
| Small | 1–3 MP | Low |
| Medium | 12–24 MP | Typical |
| Large | 40–60 MP | High |
| Stress | Synthetic | Very high mark count |

For each case record:

-   Wall-clock time per pipeline stage.
-   Peak resident memory.
-   Number of generated marks.
-   Input polygon vertex count.
-   Final path vertex count.
-   SVG byte size.
-   Geometry union time and failure status.

Run each geometry benchmark against both candidate Boolean engines using
the same generated input.

------------------------------------------------------------------------

## 13. Proposed Rust Prototype Plan

Before implementing the complete application, build a small
benchmark/prototype workspace with these experiments.

### Experiment A — Image and color pipeline

-   Decode JPEG, PNG, and TIFF using `image`.
-   Inspect orientation and ICC metadata.
-   Test `lcms2` profile conversion; compare `moxcms` (§7.8).
-   Generate a linear-light luminance plane.
-   Downsample with `fast_image_resize` to a cell-relative analysis
    resolution; measure time, peak memory, and sampling error against
    full-resolution sampling.
-   Compare known color patches and grayscale ramps.

**Decision produced:** Image decoding, ICC strategy, normalized pixel
representation, analysis resolution.

### Experiment B — AM screen geometry

-   Implement a physical Cartesian lattice.
-   Implement grid rotation and inverse transforms.
-   Implement center sampling.
-   Generate circles for a grayscale ramp.
-   Generate a preview SVG without Boolean union.

**Decision produced:** Grid coordinate model, phase model, sampling
interface, angle semantics.

### Experiment C — Coverage calibration

-   Implement isolated-circle area mapping.
-   Implement periodic circle-union coverage calculation.
-   Build a monotonic coverage-to-diameter lookup table.
-   Compare predicted and measured rasterized coverage.

**Decision produced:** Spot-growth function and error tolerance.

### Experiment D — Boolean geometry

-   Generate the same circle sets for `i_overlay` and `clipper2`.
-   Test disjoint, tangent, overlapping, and dense cases.
-   Compare correctness, runtime, peak memory, and output complexity.
-   Test holes and clipping.
-   Compare single-call union, the engine's built-in multithreading (if
    any), and tiled union with a fixed-order `rayon::join` merge
    (§12.2).
-   Rasterize results with `resvg` to check filled area and holes.

**Decision produced:** Geometry engine, numeric representation, and
union parallelization strategy.

### Experiment E — Print test chart

-   Generate 110-mesh charts at 22, 24, and 27.5 LPI.
-   Include angle variants.
-   Include tonal and minimum-feature ramps.
-   Print and record which marks survive exposure and printing.

**Decision produced:** Initial screen-print preset and physically
validated limits.

------------------------------------------------------------------------

## 14. Recommended Initial Algorithm Specification

The following is the proposed baseline for implementation after
prototype validation.

``` text
INPUT:
    source image
    physical canvas width and height
    fit mode (default: contain)
    mesh count (optional)
    LPI (optional)
    grid angle (profile default; user adjustable)
    tonal settings
    circle-size and feature constraints
    ink color

ANALYSIS:
    decode source
    apply orientation
    normalize alpha
    convert color profile to working RGB
    convert to linear-light luminance
    calculate fitted image rectangle
    recommend LPI if not explicitly supplied
    derive physical cell pitch
    enumerate rotated lattice cells
    calculate grid dimensions and recommendations
    write versioned JSON configuration

GENERATION:
    load and validate JSON
    apply CLI overrides
    decode and normalize source image
    calculate or reuse luminance representation
    enumerate lattice cells deterministically
    sample luminance at each cell center
    apply tonal response
    convert target ink coverage to circle diameter
    suppress marks below configured feature threshold
    construct circle geometry
    clip to output canvas
    union all geometry
    simplify only within documented tolerance
    serialize one compound SVG path
    validate SVG
    write output atomically
```

If analysis and generation are separate invocations, the analysis JSON
should contain enough information to reproduce the recommended settings.
It should not assume that a large raster buffer can or should be
embedded in JSON. The generation command can decode the source again,
and the configuration should optionally verify a source hash to detect
changes.

------------------------------------------------------------------------

## 15. Decisions and Open Research Questions

### Recommended now

1.  Use AM / clustered-dot screening.
2.  Use a regular Cartesian lattice.
3.  Use circular marks in the first release.
4.  Use physical cell pitch derived from LPI.
5.  Use luminance-to-coverage mapping rather than a naïve linear
    diameter mapping.
6.  Calibrate circle diameter for periodic overlap.
7.  Keep image decoding, luminance, grid, geometry, and SVG
    serialization in separate crates.
8.  Evaluate `image`, `lcms2`, Rayon, `i_overlay`, and `clipper2`.
9.  Use 22 LPI as a conservative 110-mesh starting preset, clearly
    labeled advisory.
10. Keep angle configurable; treat 22.5° as a candidate, not a universal
    rule.

### Must be decided by prototype or print test

-   Exact LPI preset and recommended range for the user's actual
    110-mesh screen.
-   Default angle.
-   Cell-center versus area-average sampling.
-   ICC profile fallback behavior for untagged images.
-   Polygon approximation tolerance.
-   Geometry engine.
-   Integer versus floating-point geometry.
-   Union batching strategy.
-   Minimum screen-print and linocut feature sizes.
-   Whether the final path is best emitted as polygonal line segments or
    curve commands.

### Important product recommendation

Keep **analysis recommendations**, **user-selected parameters**, and
**effective generation parameters** distinct in the configuration model.
This will make the tool explainable and reproducible.

For example, a user may receive a 22 LPI recommendation from a 110-mesh
screen, change it to 24 LPI, and then set a larger minimum dot size for
a particular substrate. The generated SVG should be traceable to those
decisions.

------------------------------------------------------------------------

## 16. Sources and Further Reading

The following sources informed this initial research. Vendor guidance is
useful for practical starting points but should not be treated as a
universal print standard. Crate documentation describes current APIs and
capabilities; dependency versions and features should be rechecked when
implementation begins.

### Digital halftoning

1.  **Modern Digital Halftoning**, chapter on AM halftoning. Describes
    AM screening as regular round dot clusters whose size changes with
    tone, along with spot functions and screen frequency/angle.\
    <https://api.pageplace.de/preview/DT0400.9781420047547_A23978368/preview-9781420047547_A23978368.pdf>

2.  **Lau et al., Digital halftoning research**, Journal of the Optical
    Society of America A (1999). Discusses clustered-dot AM halftoning
    and contrasts its regular lattice with other halftoning approaches.\
    <https://www.eecis.udel.edu/~arce/files/Publications/5-DigitalHaftoning.pdf>

3.  **Analysis and Reduction of Moiré Patterns**, Virginia Tech
    repository. Contains a mathematical review of halftone screens,
    periodic screen functions, and clustered-dot ordered dithering.\
    <https://vtechworks.lib.vt.edu/server/api/core/bitstreams/9b0c0b6e-0a25-4d71-bfc6-3ae342b8af36/content>

### Screen-printing mesh and angle

4.  **What mesh count should I use for halftones?**, ScreenPrinting.com
    Help Center. Gives the 4–5× mesh-to-LPI starting rule and explains
    process dependence.\
    <https://help.screenprinting.com/en-US/what-mesh-count-should-i-use-for-halftones-16556>

5.  **Halftone LPI vs Mesh Count**, PrintSeps. Provides mesh/LPI
    examples and a discussion of cell pitch and dot dimensions. Treat as
    supplemental guidance.\
    <https://printseps.com/guides/halftone-lpi-mesh-count.html>

6.  **Murakami Screen USA — Halftone Angles to Prevent Moiré**,
    technical newsletter. Discusses the relationship between mesh type,
    thread thickness, and halftone angles.\
    <https://www.murakamiscreen.com/documents/HalftoneAnglestopreventmoire.pdf>

### Rust image and color libraries

7.  **`image` crate documentation**. Unified Rust image decoding,
    encoding, and image-buffer APIs.\
    <https://docs.rs/image/latest/image/>

8.  **`lcms2` crate documentation**. Rust wrapper for Little CMS ICC
    profile handling and transforms.\
    <https://docs.rs/lcms2/latest/lcms2/>

9.  **`imageproc` crate documentation**. Image-processing library based
    on `image`; documents linear-color-space assumptions and optional
    Rayon-backed operations.\
    <https://docs.rs/imageproc/latest/imageproc/>

### Rust parallelism and geometry

10. **Rayon documentation**. Data-parallel iterators and work-stealing
    parallelism.\
    <https://docs.rs/rayon/latest/rayon/>

11. **`i_overlay` documentation**. Rust polygon Boolean operations, fill
    rules, holes, simplification, and integer/floating-point APIs.\
    <https://docs.rs/i_overlay/latest/i_overlay/>

12. **`clipper2` Rust crate documentation**. Rust bindings for polygon
    union, clipping, offsetting, and simplification using Clipper2.\
    <https://docs.rs/clipper2/latest/clipper2/>

------------------------------------------------------------------------

**End of research document — Draft 0.1**

This document is complementary to `Halftone.md`. It proposes algorithms
and candidate dependencies, while leaving decisions that require
prototypes or physical printing tests explicitly unresolved.
