# Halftone

## Technical Specification and Product Requirements

**Document status:** Draft 0.1\
**Project type:** Rust workspace / command-line application\
**Primary output:** Single-ink, positive SVG artwork\
**Primary use cases:** Screen-printing positives and linocut preparation

------------------------------------------------------------------------

## 1. Executive Summary

Halftone is a high-performance command-line application that converts
photographic images into single-color vector halftone artwork.

The application analyzes the tonal or luminance information in a
photograph, samples that information on a regular grid, and represents
it as variable-sized geometric marks. In the initial implementation, the
mark is a circle and the grid is a regular Cartesian X/Y grid. Darker
source regions produce larger ink marks; lighter regions produce smaller
marks.

The principal output is a transparent-background SVG containing positive
artwork: the vector shapes represent the ink to be printed or retained.
The preferred final geometry is a single compound vector path formed by
a Boolean union of the generated marks, rather than thousands of
independent SVG elements.

A key workflow requirement is a two-stage process:

1.  **Analyze:** inspect the source image and requested physical output
    size, then produce a reusable JSON configuration containing
    recommended grid dimensions, LPI, spacing, mark-size limits, and
    other parameters.
2.  **Generate:** load that JSON configuration, apply optional
    overrides, generate the halftone geometry, union the shapes,
    validate the result, and write the SVG.

The application is intended first for screen printing, with linocut as a
second important use case. It should favor robust, producible artwork
over maximum photographic detail.

## 2. Goals

### 2.1 Primary goals

-   Convert JPEG, PNG, and TIFF photographs into SVG halftone artwork.
-   Support both color and black-and-white source images.
-   Derive a tonal representation from the source image.
-   Generate variable-size circles on a regular Cartesian grid.
-   Map darker tones to larger circles by default.
-   Support normal and inverted tonal mapping.
-   Recommend grid and mark parameters based on physical output
    dimensions and screen-printing settings.
-   Support screen mesh count as an input to LPI recommendation.
-   Allow a user to override automatically recommended parameters.
-   Produce a transparent-background, single-ink positive.
-   Merge generated shapes into one compound path when feasible.
-   Support common US and ISO paper sizes, as well as custom dimensions
    in inches or centimeters.
-   Provide robust validation, diagnostics, tests, and performance
    benchmarks.
-   Organize the implementation as a reusable Rust workspace with
    clearly defined crate responsibilities.

### 2.2 Secondary goals

-   Provide print-oriented profiles for screen printing and linocut.
-   Make analysis output reproducible and reusable.
-   Keep the core processing and geometry functionality usable by
    applications other than the CLI.
-   Leave room for future pattern and primitive types without requiring
    a redesign of the core pipeline.

### 2.3 Non-goals for the initial release

-   Full-color or multi-ink separations.
-   CMYK process printing.
-   Photorealistic vector tracing.
-   Arbitrary artistic illustration or path tracing.
-   Automatic facial recognition or semantic subject preservation.
-   Stochastic or adaptive mark placement.
-   Hexagonal or isometric grids.
-   A graphical user interface.
-   Direct control of a printer or screen-printing machine.

## 3. Core Concepts and Terminology

  -----------------------------------------------------------------------
  Term                                Meaning
  ----------------------------------- -----------------------------------
  Source image                        Original JPEG, PNG, or TIFF
                                      photograph

  Luminance                           Perceptual brightness information
                                      derived from the source image

  Tonal mapping                       Transformation from source
                                      luminance to a mark-size value

  Grid                                Regular arrangement of sample
                                      positions

  LPI                                 Lines per inch; the nominal
                                      frequency of the halftone grid

  Mesh count                          Screen mesh count, conventionally
                                      expressed as threads per inch in
                                      the relevant screen-printing market

  Mark                                A geometric primitive representing
                                      ink; initially a circle

  Positive artwork                    Dark/ink geometry on a transparent
                                      background

  Compound path                       One SVG path containing multiple
                                      subpaths

  Boolean union                       Geometry operation that merges
                                      overlapping shapes into a combined
                                      filled region

  Analysis configuration              JSON document containing source
                                      metadata, recommendations, and
                                      generation settings
  -----------------------------------------------------------------------

The application should use explicit names and units in APIs and
configuration. Avoid ambiguous fields such as `size`, `resolution`, or
`density` when a more precise name is available.

## 4. User Workflow

### 4.1 Analyze an image

The user supplies an image and physical output dimensions. Optional
inputs include screen mesh count, LPI, print profile, and image-fitting
behavior.

Example:

``` bash
halftone analyze portrait.tif \
  --width 8 \
  --height 10 \
  --units in \
  --mesh 110 \
  --profile screen-print \
  --output portrait.halftone.json
```

The analysis operation should:

1.  Read and validate the source image.
2.  Inspect image dimensions, orientation, color model, and alpha
    channel.
3.  Determine the image region that will be placed on the output canvas.
4.  Convert the image to a normalized luminance representation.
5.  Evaluate the requested print dimensions.
6.  Recommend an LPI when one has not been explicitly supplied.
7.  Derive grid spacing and grid dimensions.
8.  Recommend minimum and maximum circle diameters.
9.  Record all recommendations and their rationale in JSON.
10. Write the JSON configuration without generating the final SVG.

The analysis command should not silently discard user-provided values.
The JSON should distinguish automatically recommended values from
explicitly supplied values.

### 4.2 Inspect and edit the configuration

The user can inspect the generated JSON and modify supported settings.
The JSON document should be a complete, reusable configuration for
generation, not merely a report of analysis results.

The configuration should include a schema version so future releases can
evolve it safely.

### 4.3 Generate the SVG

Example:

``` bash
halftone generate portrait.halftone.json \
  --output portrait.svg
```

Overrides should be possible without requiring the user to edit the
JSON:

``` bash
halftone generate portrait.halftone.json \
  --output portrait.svg \
  --angle 22.5 \
  --ink "#111111"
```

The application should report the effective configuration, including any
command-line overrides, in verbose mode.

### 4.4 Optional one-step operation

A convenience command may perform analysis and generation in one
invocation:

``` bash
halftone convert portrait.jpg \
  --width 8 \
  --height 10 \
  --units in \
  --mesh 110 \
  --output portrait.svg
```

The one-step operation must use the same analysis and generation APIs as
the two-stage workflow. It must not implement a separate processing
path.

## 5. Input Image Requirements

### 5.1 Supported formats

Initial formats:

-   JPEG
-   PNG
-   TIFF

The image-decoding layer should hide format-specific details from the
processing crates.

### 5.2 Image normalization

Normalize decoded input to a documented internal pixel representation.
The exact representation should be selected after evaluating Rust
image-processing crates and their performance characteristics.

Requirements:

-   Correctly handle common RGB, grayscale, and RGBA inputs.
-   Respect embedded orientation metadata where supported.
-   Define behavior for images with alpha transparency.
-   Handle 8-bit and higher-bit-depth inputs where supported by the
    selected decoder.
-   Avoid unnecessary image copies.
-   Validate dimensions and allocation sizes before allocating large
    buffers.
-   Produce consistent luminance values regardless of the input file
    format, subject to the color-management capabilities of the
    implementation.

### 5.3 Alpha-channel behavior

The initial default should composite transparent pixels against a
configurable analysis background, with white as the recommended default
for a positive-print workflow. The configuration must record the
selected behavior.

This avoids treating transparent pixels as arbitrary black or allowing
decoder-specific behavior to affect the result.

### 5.4 Color management

Color photographs should be converted to a single luminance channel.
Research and implementation planning must evaluate:

-   Embedded ICC profile handling.
-   Conversion to a known working color space.
-   Linear-light versus gamma-encoded calculations.
-   Perceptual luminance calculation.
-   Grayscale and monochrome source handling.

The implementation should document its color-management assumptions. It
should not imply colorimetric accuracy when an input profile is absent
or unsupported.

## 6. Tonal Processing

### 6.1 Default tonal direction

The default mapping is:

-   Dark source tones produce larger ink circles.
-   Light source tones produce smaller ink circles.
-   The background remains transparent.
-   The ink color is configurable and defaults to black.

An inversion option should reverse the tonal mapping while retaining the
positive-artwork output model. Inversion changes which regions receive
larger or smaller marks; it does not automatically create a solid
background.

### 6.2 Luminance calculation

The analysis pipeline should derive a normalized luminance value for
each source sample.

The research phase must select the preferred luminance method.
Considerations include:

-   Correct treatment of sRGB transfer functions.
-   Conversion into linear-light RGB before weighted luminance
    calculation.
-   A perceptual luminance model such as Rec. 709 / sRGB primaries.
-   Optional alternate grayscale methods only if they materially improve
    artistic control.

The internal convention should be explicit, for example:

-   `0.0` = black
-   `1.0` = white

### 6.3 Tonal response

The conversion from luminance to circle diameter should be configurable
and should not be limited to a linear mapping.

Potential response controls:

-   Contrast.
-   Brightness or tonal offset.
-   Gamma.
-   Black point.
-   White point.
-   Optional curve or preset in a future version.

A simple monotonic response function is appropriate for the first
implementation. It must be deterministic and tested at boundary values.

### 6.4 Minimum and maximum circle diameter

The analysis stage should recommend physical minimum and maximum circle
diameters. The user may override them.

The implementation must define behavior when:

-   The minimum diameter is zero.
-   The maximum diameter exceeds the grid-cell dimensions.
-   A circle would overlap neighboring circles.
-   A tonal value maps below the minimum mark threshold.
-   A tonal value maps above the maximum mark threshold.

The recommended configuration should avoid accidental geometry overlap
unless overlap is deliberately selected. The geometry-union stage must
still correctly handle intended overlaps.

### 6.5 Mark suppression

A configurable threshold may suppress marks below a minimum useful size.
Suppression is important for print production and can prevent tiny
isolated shapes that are difficult to expose, print, or carve.

The threshold should be expressed in physical units and recorded in the
configuration.

## 7. Grid and Sampling

### 7.1 Initial grid

The first release uses a regular Cartesian grid with X/Y sampling.

Requirements:

-   Uniform horizontal and vertical spacing.
-   Configurable grid angle.
-   Variable circle diameter at each grid location.
-   Stable, deterministic sample placement.
-   Explicit grid origin and coverage behavior.
-   Grid rows and columns recorded in the analysis JSON.

The initial default angle is a proposed screen-printing convention and
must be confirmed through research and printing tests. A 22.5-degree
default is a candidate, not yet a finalized requirement.

### 7.2 Grid dimensions

The analysis stage should recommend the number of columns and rows based
on:

-   Physical output dimensions.
-   Selected LPI.
-   Grid angle.
-   Image aspect ratio and fit behavior.
-   Any required margins or safe area.

The relationship between LPI, rotation, and grid spacing must be defined
carefully. Do not assume that a simple `width × LPI` calculation is
sufficient for a rotated grid or for both axes on a non-square canvas.

The JSON should record both the logical grid dimensions and the physical
spacing used by the generator. The implementation must document whether
the configured LPI represents frequency along the grid axes or another
convention.

### 7.3 Future grid types

The architecture should allow additional grid strategies without
changing tonal analysis or SVG serialization.

Future candidates:

-   Hexagonal grid.
-   Isometric grid.
-   Stochastic distribution.
-   Adaptive distribution.

These are explicitly outside the initial implementation scope.

## 8. Screen-Printing Recommendations

### 8.1 LPI and mesh count

The CLI should support:

-   Explicit LPI.
-   Screen mesh count.
-   Automatic LPI recommendation based on mesh count.
-   A manual LPI override that takes precedence over automatic
    recommendation.
-   Named print presets.

The initial reference screen is 110 mesh.

A rough screen-printing rule of thumb sometimes uses a mesh-to-LPI ratio
in the range of approximately 4:1 to 5:1. For a 110-mesh screen this
implies a preliminary range of roughly 22--27.5 LPI. This is a research
starting point only, not a guaranteed production setting.

The final recommendation must account for more than mesh count. Research
should investigate:

-   Mesh count and thread diameter.
-   Mesh opening.
-   Screen tension and mesh material.
-   Emulsion and stencil characteristics.
-   Ink deposit and ink properties.
-   Substrate.
-   Image angle and interaction with mesh.
-   Printer capability and exposure quality.

The application should communicate that mesh-based LPI recommendations
are advisory. It cannot guarantee printability from mesh count alone.

### 8.2 Presets

Initial profile:

-   `screen-print`

Potential additional profile:

-   `linocut`

The screen-print profile should provide a conservative starting
configuration for a 110-mesh screen. Exact default LPI and mark-size
values must be finalized after research and practical testing.

### 8.3 Angle

The application should expose a user-adjustable angle. A common
screen-printing angle should be supplied as the default once validated.

The angle must be represented in degrees and normalized consistently.
The configuration should make clear whether the angle refers to the grid
orientation or the screen/halftone relationship.

## 9. Physical Output and Image Fitting

### 9.1 Units

Support:

-   Inches (`in`)
-   Centimeters (`cm`)

Internally, use a single canonical physical unit or a well-defined
physical coordinate system. Convert only at input and output boundaries
where possible.

### 9.2 Paper sizes

Provide named presets for common US and ISO paper sizes. The initial
list should include commonly used sizes, with a mechanism to add more
without changing the processing pipeline.

Examples:

**US** - Letter - Legal - Tabloid

**ISO** - A4 - A3 - A5

The user may also supply arbitrary width and height values.

### 9.3 Fit behavior

Default: fit the entire photograph inside the requested output canvas
while preserving its aspect ratio.

The image must not be stretched to fill the canvas.

If aspect ratios differ, the remaining area should be treated as
unprinted/transparent. The configuration should record the placement and
any resulting margins.

Optional future or initial alternative:

-   `crop`: crop the source to fill the canvas.

The exact crop anchor should be configurable if crop mode is
implemented.

### 9.4 SVG physical dimensions

The exported SVG should include physical `width` and `height` attributes
in the selected unit, along with a suitable `viewBox`.

The geometry should be defined in a consistent coordinate system so that
scaling the SVG preserves the intended physical relationships.

## 10. Vector Geometry and SVG Output

### 10.1 Positive artwork

The SVG should contain ink shapes on a transparent background.

Requirements:

-   One configurable fill color.
-   No required background rectangle.
-   No embedded source raster image.
-   No separate color layers.
-   No unnecessary SVG metadata or editor-specific objects.
-   A valid `viewBox` and physical dimensions.

### 10.2 Geometry union

The preferred output is a single compound path produced by Boolean union
of all generated marks.

Benefits:

-   One logical ink region.
-   Overlapping marks are merged.
-   Easier downstream handling for print production and relief-print
    preparation.
-   Reduced SVG element count.

The implementation must preserve holes and disconnected components
correctly. It must use an appropriate fill rule, such as `nonzero` or
`evenodd`, consistently with the geometry library's output.

A single SVG `<path>` can contain multiple disconnected subpaths. This
is acceptable and is not the same as requiring the entire design to be
one physically connected component.

### 10.3 Union performance

Boolean union can be one of the most expensive stages, especially for
high-resolution images with many marks.

The research phase must evaluate Rust geometry libraries for:

-   Robust polygon Boolean operations.
-   Floating-point stability.
-   Handling of touching and overlapping circles.
-   Large collections of primitives.
-   Output path simplification.
-   Memory use.
-   Licensing and maintenance.
-   Availability of reliable tests and documentation.

Do not assume that unioning every primitive in one naive operation will
scale. Benchmark alternative strategies, including batching or spatial
grouping, while preserving exact final geometry.

### 10.4 SVG serialization

The SVG crate should:

-   Serialize valid SVG.
-   Write a single fill color.
-   Write physical dimensions and `viewBox`.
-   Avoid unnecessary precision while preserving geometric accuracy.
-   Support deterministic output where practical.
-   Validate or sanity-check the final document before reporting
    success.

## 11. Linocut-Oriented Constraints

Linocut is an important secondary use case. Its physical constraints
differ from screen printing and should be represented in the
architecture even if some are implemented after the first release.

Potential controls:

-   Minimum retained feature width.
-   Minimum gap between retained shapes.
-   Removal of isolated tiny marks.
-   Contour simplification.
-   Maximum geometry complexity.
-   Optional merging of nearby shapes.
-   A linocut-oriented preset.

These controls must be defined in terms of physical output dimensions,
not arbitrary pixel thresholds.

The implementation must be careful about terminology: minimum ink
feature size and minimum gap between ink features are different
constraints. Both may matter for a positive intended for relief
printing.

The first release may provide basic minimum-feature and minimum-gap
validation without implementing every possible geometry repair
operation.

## 12. Configuration JSON

### 12.1 Requirements

The analysis JSON is a first-class product artifact.

It must be:

-   Human-readable.
-   Editable.
-   Versioned.
-   Strictly validated.
-   Reusable by the generation command.
-   Explicit about physical units.
-   Explicit about automatic recommendations versus user overrides.
-   Sufficient to reproduce generation when paired with the same source
    image and compatible application version.

### 12.2 Illustrative schema

The following is an illustrative structure, not a finalized JSON Schema:

``` json
{
  "schema_version": 1,
  "source": {
    "path": "portrait.tif",
    "width_px": 4000,
    "height_px": 6000,
    "orientation_applied": true,
    "analysis_background": "#FFFFFF"
  },
  "output": {
    "width": 8.0,
    "height": 10.0,
    "units": "in",
    "fit": "contain",
    "ink_color": "#000000"
  },
  "profile": "screen-print",
  "grid": {
    "type": "cartesian",
    "angle_degrees": 22.5,
    "columns": 176,
    "rows": 220,
    "spacing_x": 0.04545,
    "spacing_y": 0.04545,
    "spacing_units": "in"
  },
  "halftone": {
    "shape": "circle",
    "tonal_direction": "dark_larger",
    "lpi": 22,
    "mesh_count": 110,
    "minimum_diameter": 0.01,
    "maximum_diameter": 0.036,
    "diameter_units": "in",
    "tonal_gamma": 1.0,
    "contrast": 1.0,
    "minimum_feature_width": 0.0,
    "minimum_gap": 0.0
  },
  "recommendations": {
    "lpi_source": "mesh-derived",
    "lpi_recommended": 22,
    "notes": [
      "Recommendation is advisory; validate with the actual screen, ink, and substrate."
    ]
  }
}
```

### 12.3 Configuration validation

Validation should reject or clearly warn about:

-   Unsupported schema versions.
-   Missing required fields.
-   Unknown enum values.
-   Invalid units.
-   Non-finite numbers (`NaN`, positive or negative infinity).
-   Negative dimensions.
-   Zero or impossible grid dimensions.
-   Minimum diameter greater than maximum diameter.
-   Invalid colors.
-   Invalid angles or unsupported angle ranges.
-   LPI and mesh values outside defined safe ranges.
-   Inconsistent grid spacing and grid dimensions.
-   Source file missing or unreadable.

The schema should define which values may be omitted and which defaults
are applied. Silent fallback should be avoided for settings that
materially affect print geometry.

### 12.4 Reproducibility

The configuration should record:

-   Schema version.
-   Application version.
-   Source image metadata.
-   Effective generation settings.
-   Analysis recommendations.
-   User overrides, where practical.

The application should consider recording a source-file hash so users
can detect when the original image has changed.

## 13. CLI Design

The CLI should use subcommands and provide clear help output.

Proposed commands:

``` text
halftone analyze <INPUT> [OPTIONS]
halftone generate <CONFIG> [OPTIONS]
halftone convert <INPUT> [OPTIONS]
halftone validate <CONFIG>
halftone version
```

### 13.1 Analyze options

Potential options:

``` text
--width <VALUE>
--height <VALUE>
--units <in|cm>
--paper <NAME>
--fit <contain|crop>
--profile <screen-print|linocut>
--mesh <COUNT>
--lpi <VALUE>
--angle <DEGREES>
--ink <COLOR>
--output <FILE>
```

### 13.2 Generate options

Potential options:

``` text
--output <FILE>
--angle <DEGREES>
--lpi <VALUE>
--ink <COLOR>
--min-diameter <VALUE>
--max-diameter <VALUE>
--gamma <VALUE>
--contrast <VALUE>
--verbose
```

CLI overrides should be validated using the same validation code as JSON
configuration values.

### 13.3 Diagnostics

Errors should be actionable and identify:

-   The file or setting involved.
-   The invalid value.
-   The expected range or format.
-   A suggested correction when possible.

Use appropriate exit codes for invalid input, invalid configuration,
processing failure, output failure, and internal errors.

## 14. Rust Workspace Architecture

### 14.1 Design principles

-   Separate reusable libraries from CLI orchestration.
-   Keep dependencies flowing in one direction.
-   Avoid circular crate dependencies.
-   Keep public APIs small and documented.
-   Keep image decoding separate from tonal algorithms.
-   Keep geometry construction separate from SVG serialization.
-   Avoid leaking third-party geometry or image-library types
    unnecessarily through public APIs.
-   Prefer explicit domain types and units over untyped numeric values.
-   Use safe Rust by default.
-   Introduce unsafe code only with a demonstrated need, documentation,
    and tests.
-   Keep algorithms deterministic where practical.

### 14.2 Proposed workspace

``` text
halftone/
├── Cargo.toml
├── Cargo.lock
├── rust-toolchain.toml
├── README.md
├── docs/
│   ├── architecture.md
│   ├── algorithms.md
│   └── configuration.md
├── crates/
│   ├── halftone-types/
│   ├── halftone-image/
│   ├── halftone-core/
│   ├── halftone-grid/
│   ├── halftone-analysis/
│   ├── halftone-geometry/
│   ├── halftone-svg/
│   └── halftone-cli/
├── tests/
│   └── fixtures/
└── benches/
```

The exact crate boundaries should be reviewed during research. Avoid
creating tiny crates solely for the sake of separation; each crate
should represent a meaningful reusable responsibility.

### 14.3 Crate responsibilities

#### `halftone-types`

Shared domain types:

-   Physical dimensions and units.
-   Pixel dimensions.
-   Luminance values.
-   Grid configuration.
-   Print profiles.
-   Shape parameters.
-   Tonal mapping settings.
-   Configuration data structures.
-   Shared error types where appropriate.

This crate should have minimal dependencies.

#### `halftone-image`

Responsibilities:

-   JPEG, PNG, and TIFF decoding.
-   Orientation handling.
-   Pixel-format normalization.
-   Alpha handling.
-   Color-profile handling, subject to selected library support.
-   Image metadata extraction.

It should expose normalized image data to the rest of the workspace.

#### `halftone-core`

Responsibilities:

-   Luminance conversion.
-   Tonal transformations.
-   Gamma and contrast.
-   Normal and inverted tonal mapping.
-   Conversion from luminance to primitive dimensions.

This crate should not depend on the CLI or SVG serializer.

#### `halftone-grid`

Responsibilities:

-   Cartesian grid creation.
-   Grid rotation.
-   Sample coordinate generation.
-   Mapping between physical output coordinates and source image
    coordinates.
-   Grid dimension calculations.

Future grid implementations should satisfy a common interface or
strategy abstraction.

#### `halftone-analysis`

Responsibilities:

-   Image and print-size analysis.
-   LPI recommendation.
-   Mesh-based recommendations.
-   Grid-size recommendations.
-   Feature-size recommendations.
-   Analysis report and configuration generation.
-   Configuration validation and versioning.

#### `halftone-geometry`

Responsibilities:

-   Primitive construction.
-   Circle approximation or native curve representation, depending on
    the Boolean library.
-   Shape clipping to the output canvas.
-   Geometry union.
-   Geometry simplification.
-   Minimum-feature and minimum-gap checks or processing.

#### `halftone-svg`

Responsibilities:

-   SVG document construction.
-   Compound path serialization.
-   Physical dimensions and `viewBox`.
-   Fill color and fill rule.
-   SVG output validation.

#### `halftone-cli`

Responsibilities:

-   Argument parsing.
-   User-facing commands.
-   Configuration loading and saving.
-   Workflow orchestration.
-   Progress and diagnostic reporting.
-   Exit codes.

### 14.4 Dependency direction

A preferred dependency direction is:

``` text
halftone-cli
 ├── halftone-analysis
 ├── halftone-image
 ├── halftone-core
 ├── halftone-grid
 ├── halftone-geometry
 └── halftone-svg

halftone-analysis
 ├── halftone-types
 ├── halftone-image
 ├── halftone-core
 └── halftone-grid

halftone-geometry
 └── halftone-types

halftone-svg
 └── halftone-types

halftone-core
 └── halftone-types

halftone-grid
 └── halftone-types

halftone-image
 └── halftone-types
```

This is a conceptual dependency diagram. The implementation should avoid
dependencies that are not needed by a crate's public responsibility.

## 15. Rust Dependency Research

Do not finalize dependencies before evaluating current crate health,
APIs, performance, maintenance, and licensing.

Research candidates in the following categories.

  -----------------------------------------------------------------------
  Capability                          Candidate crates / areas to
                                      investigate
  ----------------------------------- -----------------------------------
  Image decoding                      `image`, specialized TIFF/JPEG/PNG
                                      decoders

  Color management                    Rust ICC/CMM ecosystem; Little CMS
                                      bindings if justified

  Parallel processing                 `rayon`

  CLI parsing                         `clap`

  Serialization                       `serde`, `serde_json`

  Error handling                      `thiserror`, `anyhow` at
                                      application boundaries

  Geometry                            `geo`, `geo-types`, `i_overlay`,
                                      `clipper2` bindings or alternatives

  SVG output                          SVG serialization crates or
                                      carefully controlled custom
                                      serialization

  Testing                             `proptest`, `insta` where useful

  Benchmarking                        `criterion`
  -----------------------------------------------------------------------

The research should compare actual suitability rather than selecting the
most popular crate automatically.

### 15.1 Image processing evaluation

Evaluate:

-   Decoder coverage and bit-depth support.
-   Memory behavior.
-   Color-profile support.
-   Orientation handling.
-   SIMD opportunities.
-   Compatibility with Rayon or other parallel processing.
-   Whether the library copies or converts image buffers unnecessarily.

### 15.2 Geometry evaluation

This is a critical research task.

Compare Boolean geometry libraries for:

-   Union of large collections of circles or polygonal approximations.
-   Numerical robustness.
-   Handling of tangencies and coincident boundaries.
-   Hole preservation.
-   Output validity.
-   Simplification.
-   Runtime and peak memory.
-   API ergonomics.
-   License compatibility.

Benchmark realistic workloads, not just small synthetic examples.

### 15.3 SVG serialization evaluation

Compare a dedicated SVG crate against direct serialization of a small,
controlled SVG subset.

The selected approach should support:

-   One compound path.
-   Explicit fill color.
-   Physical width and height.
-   `viewBox`.
-   Correct fill-rule behavior.
-   Deterministic and compact output.

## 16. Performance Requirements

Performance is a first-class requirement, but correctness and geometry
robustness take priority over premature optimization.

### 16.1 Pipeline stages

Measure these independently:

1.  Image decoding.
2.  Image normalization and color conversion.
3.  Luminance generation.
4.  Image sampling.
5.  Tonal mapping.
6.  Primitive generation.
7.  Boolean union.
8.  Geometry simplification.
9.  SVG serialization.

### 16.2 Parallelization

Parallel processing candidates:

-   Pixel normalization.
-   Luminance conversion.
-   Independent grid sample evaluation.
-   Primitive generation.
-   Independent preprocessing operations.

Boolean union parallelization must be evaluated against the chosen
geometry library. Do not assume that parallelizing it is safe or
beneficial.

### 16.3 Memory management

Requirements:

-   Validate dimensions before allocation.
-   Avoid unnecessary full-size intermediate buffers.
-   Use compact representations for luminance and sample data where
    appropriate.
-   Avoid cloning large image buffers.
-   Consider streaming or tiled processing if image sizes demand it.
-   Bound geometry growth and detect excessive output complexity.

### 16.4 Benchmarks

Create repeatable benchmarks using:

-   Small image.
-   Medium photographic image.
-   Large photographic image.
-   Low-detail image.
-   High-detail image.
-   Dense halftone output.
-   Geometry with many overlapping marks.
-   Geometry with mostly disconnected marks.

Track runtime and peak memory for each major stage.

## 17. Testing and Validation Strategy

Testing must be built into the architecture from the beginning, not
deferred until after the algorithms are complete.

### 17.1 Unit tests

Each crate should have focused unit tests for its public and important
internal behavior.

Examples:

-   Luminance calculation at known RGB values.
-   Gamma and contrast boundaries.
-   Dark-to-large and light-to-small mapping.
-   Inverted tonal mapping.
-   Grid coordinate calculations.
-   Rotated grid placement.
-   Unit conversions.
-   LPI recommendation calculations.
-   JSON parsing and validation.
-   Geometry union.
-   SVG physical dimensions and path output.

### 17.2 Property-based tests

Use property-based tests where useful.

Examples:

-   Tonal mapping remains within the configured diameter range.
-   A monotonic tonal mapping remains monotonic.
-   Grid dimensions never become negative or overflow.
-   Unit conversion round trips remain within a defined tolerance.
-   Valid generated geometry has finite coordinates.
-   Boolean union preserves the intended filled region.
-   Configuration serialization and deserialization preserve effective
    settings.

### 17.3 Integration tests

End-to-end fixtures should cover:

-   JPEG color photograph.
-   PNG color photograph.
-   TIFF photograph.
-   Grayscale input.
-   RGBA input.
-   Portrait and landscape aspect ratios.
-   Contain/fit behavior.
-   Normal and inverted tonal mapping.
-   Explicit LPI.
-   Mesh-derived LPI.
-   Manual overrides.
-   SVG output with one compound path.

### 17.4 Regression tests

Keep a small, licensed fixture set with known expected properties.

Avoid relying exclusively on exact byte-for-byte SVG comparisons because
geometry libraries and serializers may change harmless output ordering
or floating-point formatting.

Prefer assertions on:

-   SVG structure.
-   Physical dimensions.
-   Number of subpaths or components where stable.
-   Bounding boxes.
-   Filled area within tolerance.
-   Geometry validity.
-   Configuration values.
-   Selected rasterized reference comparisons.

### 17.5 Sanity checks

The application should check:

-   Input file exists and is readable.
-   Input format is supported.
-   Dimensions are within configured safety limits.
-   Output dimensions are positive and finite.
-   LPI and mesh count are valid.
-   Grid dimensions are plausible.
-   Circle diameters are finite and ordered.
-   Coordinates remain within a safe numerical range.
-   Geometry union succeeds.
-   Output path is writable.
-   SVG is structurally valid.

Warnings should be distinct from errors. For example, an aggressive LPI
recommendation may warrant a warning, while an invalid physical
dimension should be an error.

## 18. Error Handling and Safety

-   Use typed errors in reusable crates.
-   Add contextual error messages at workflow boundaries.
-   Avoid panics for malformed input or user configuration.
-   Validate external data before processing.
-   Prevent integer overflow in dimension and buffer calculations.
-   Set configurable limits for input pixels, sample count, and geometry
    complexity.
-   Handle interrupted writes safely where practical.
-   Prefer writing to a temporary output and renaming on success so
    failed generation does not leave a misleading partial SVG.
-   Do not silently produce an incomplete SVG after a geometry failure.

## 19. Output Validation

Before reporting success, validate the generated SVG.

At minimum:

-   XML/SVG document is well-formed.
-   Physical width and height are present.
-   `viewBox` is valid.
-   Fill color is valid.
-   Background is transparent unless explicitly requested otherwise in a
    future feature.
-   No embedded raster image exists.
-   No unexpected additional layers or colors exist.
-   The intended compound path exists.
-   Coordinates are finite.
-   Geometry is within the output canvas, subject to documented clipping
    behavior.

A separate `validate` command may validate a configuration and,
optionally, an existing SVG.

## 20. Development Phases

### Phase 0 --- Research and technical decisions

Deliverables:

-   Rust crate comparison.
-   Geometry Boolean library evaluation.
-   Image decoding and color-management evaluation.
-   LPI and mesh-count research.
-   Grid-angle research.
-   Tonal mapping algorithm proposal.
-   SVG representation strategy.
-   Benchmark plan.
-   Final workspace dependency graph.

Exit criteria: Major technical dependencies and algorithms have
evidence-based selections.

### Phase 1 --- Workspace and foundational types

Deliverables:

-   Cargo workspace.
-   Crate skeletons.
-   Shared domain types.
-   Unit and coordinate types.
-   Error conventions.
-   CI checks.
-   Initial unit tests.

Exit criteria: Workspace builds, tests, and documents cleanly.

### Phase 2 --- Image ingestion and tonal analysis

Deliverables:

-   JPEG/PNG/TIFF input.
-   Orientation and alpha handling.
-   Normalized image representation.
-   Luminance conversion.
-   Tonal mapping.
-   Analysis report.

Exit criteria: Deterministic luminance and tonal results for fixture
images.

### Phase 3 --- Grid and recommendations

Deliverables:

-   Cartesian grid.
-   Grid rotation.
-   Physical coordinate calculations.
-   Mesh-based LPI recommendation.
-   Grid dimension recommendation.
-   Circle-size recommendations.
-   Versioned JSON configuration.

Exit criteria: Analysis command produces a valid, reusable
configuration.

### Phase 4 --- Geometry and SVG

Deliverables:

-   Circle geometry.
-   Canvas clipping.
-   Boolean union.
-   Compound path generation.
-   SVG serialization.
-   Output validation.

Exit criteria: Valid single-color positive SVG from supported input
fixtures.

### Phase 5 --- Print-production controls

Deliverables:

-   Screen-print preset.
-   Mesh and LPI controls.
-   Physical feature thresholds.
-   Minimum gap checks.
-   Linocut-oriented controls or initial validation.
-   More complete diagnostic messages.

Exit criteria: Settings and output are suitable for controlled
print-production testing.

### Phase 6 --- Performance and hardening

Deliverables:

-   Benchmarks.
-   Parallel processing improvements.
-   Memory profiling.
-   Geometry stress tests.
-   Regression fixtures.
-   Documentation and release packaging.

Exit criteria: Performance is measured, major failure modes are covered,
and release behavior is documented.

## 21. Open Decisions

The following decisions should remain explicitly open until research or
practical testing provides sufficient evidence.

1.  **Default screen angle:** 22.5 degrees is a candidate, but the
    correct convention and mesh interaction must be validated.
2.  **Default LPI for 110 mesh:** Approximately 22--24 LPI is a
    conservative starting hypothesis, not a final value.
3.  **LPI interpretation:** Define precisely how LPI relates to grid
    spacing, rotation, and physical output dimensions.
4.  **Luminance model:** Select the color-space conversion and luminance
    calculation after color-management research.
5.  **Alpha handling:** Confirm the default analysis background and
    whether additional alpha modes are needed.
6.  **Geometry engine:** Select after benchmarks of Boolean union
    performance and robustness.
7.  **Circle representation:** Determine whether circles should remain
    native curves through the geometry stage or be approximated with
    polygons for Boolean operations.
8.  **Geometry simplification:** Establish safe tolerances in physical
    units.
9.  **Linocut controls:** Decide which minimum-feature, minimum-gap, and
    cleanup operations belong in the first release.
10. **Paper-size catalog:** Confirm the initial set of US and ISO sizes.
11. **Crop behavior:** Decide whether crop mode belongs in the first
    release or a later iteration.
12. **Configuration source handling:** Decide whether generation should
    require the original source path to remain available or whether the
    analysis stage should optionally cache normalized image data.

## 22. Acceptance Criteria for the Initial Release

The initial release is complete when a user can:

1.  Supply a supported JPEG, PNG, or TIFF photograph.
2.  Specify a standard paper size or custom physical dimensions.
3.  Analyze the photograph and receive a JSON configuration.
4.  Receive a recommended grid size and screen-printing LPI based on the
    selected mesh count.
5.  Edit the JSON configuration or override supported values on the CLI.
6.  Generate a regular Cartesian grid of variable-size circles.
7.  Produce positive artwork with dark source areas represented by
    larger ink circles.
8.  Select normal or inverted tonal mapping.
9.  Generate a transparent-background SVG with a configurable single ink
    color.
10. Produce merged compound-path geometry through Boolean union.
11. Validate the configuration and final SVG.
12. Run the test suite and benchmark the major processing stages.

The workspace must be modular, reusable, documented, and covered by
unit, integration, and regression tests.

## 23. Research Principles

The implementation phase should follow these principles:

-   Prefer maintained, well-documented Rust crates with compatible
    licenses.
-   Benchmark critical dependencies using representative workloads.
-   Favor simple, mathematically clear algorithms for the initial
    release.
-   Keep physical units explicit throughout the pipeline.
-   Treat print-production recommendations as advisory unless validated
    under real production conditions.
-   Avoid unnecessary abstraction while preserving clear crate
    boundaries.
-   Make configuration and output deterministic wherever practical.
-   Optimize only after correctness and profiling.
-   Keep the geometry pipeline robust; a visually plausible but invalid
    SVG is not an acceptable result.

------------------------------------------------------------------------

**End of specification --- Draft 0.1**
