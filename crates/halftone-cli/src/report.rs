//! Human-readable reports (stderr for diagnostics, stdout for results).

use halftone::{Config, Generated};
use std::path::Path;

fn len(c: &Config, mm: f64) -> String {
    let u = c.output.display_units;
    format!("{:.2} {}", u.from_mm(mm), u.suffix())
}

pub fn analysis(c: &Config, path: &Path) {
    let s = &c.screen;
    println!("wrote {}", path.display());
    println!(
        "  source   {} ({}x{} px, {})",
        c.source.path, c.source.width_px, c.source.height_px, c.source.color
    );
    println!(
        "  page     {} x {} (paper {} x {} minus {} margins)",
        len(c, c.layout.page_width_mm),
        len(c, c.layout.page_height_mm),
        len(c, c.output.paper_width_mm),
        len(c, c.output.paper_height_mm),
        len(c, c.output.printer_margin_mm)
    );
    println!(
        "  image    {} x {}",
        len(c, c.layout.image.width_mm),
        len(c, c.layout.image.height_mm)
    );
    let range = c
        .recommendations
        .lpi_range
        .map(|[a, b]| format!(", advisory {a:.1}..{b:.1}"))
        .unwrap_or_default();
    println!(
        "  screen   {} lpi ({:?}{range}) at {}°, {} cells",
        s.lpi, c.recommendations.lpi_source, s.angle_deg, c.layout.cells
    );
    for n in &c.recommendations.notes {
        println!("  note     {n}");
    }
}

pub fn summary(g: &Generated, svg: &Path, verbose: bool) {
    let st = &g.stats;
    println!(
        "wrote {} ({:.1} MB, {} marks)",
        svg.display(),
        g.svg.len() as f64 / 1e6,
        st.marks
    );
    if !verbose {
        return;
    }
    let c = &g.config;
    let t = &g.timings;
    eprintln!("effective settings:");
    eprintln!(
        "  lpi {} at {}°, pitch {:.4} mm, {} cells",
        c.screen.lpi, c.screen.angle_deg, c.layout.pitch_mm, c.layout.cells
    );
    eprintln!(
        "  tone: invert {}, black {}, white {}, gamma {}, min dot {} mm, max coverage {}",
        c.tone.invert, c.tone.black_point, c.tone.white_point, c.tone.gamma, c.tone.min_dot_mm, c.tone.max_coverage
    );
    eprintln!("  user-set: {}", c.user_set.join(", "));
    eprintln!(
        "geometry: {} isolated, {} overlapping -> {} polygons, {} points",
        st.isolated,
        st.overlapping,
        g.artwork.0.len(),
        st.coords
    );
    eprintln!(
        "timings: decode {:.0?}, downscale {:.0?}, marks {:.0?}, geometry {:.0?}, svg {:.0?}",
        t.decode, t.downscale, t.marks, t.geometry, t.svg
    );
}
