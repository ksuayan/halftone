//! Output validation (spec S§19): structure, size, colour, and path data that
//! parse to finite coordinates inside the page.

use halftone_types::units::UM_PER_MM;

pub struct Expect<'a> {
    pub page_width_mm: f64,
    pub page_height_mm: f64,
    pub ink: &'a str,
    pub registration_marks: bool,
}

#[derive(Debug, Default, PartialEq)]
pub struct SvgReport {
    pub subpaths: usize,
    pub coords: usize,
}

fn length_mm(v: Option<&str>) -> Option<f64> {
    v?.strip_suffix("mm")?.parse::<f64>().ok().filter(|x| x.is_finite())
}

/// Walk path data made of M/m, L/l, H/h, V/v, A/a and Z/z; returns
/// (subpaths, coordinates) or the first problem found.
fn walk_path(d: &str, w: f64, h: f64, allow_arcs: bool) -> Result<(usize, usize), String> {
    let mut nums: Vec<f64> = Vec::new();
    let mut cmd = ' ';
    let (mut x, mut y, mut sx, mut sy) = (0.0, 0.0, 0.0, 0.0);
    let (mut subpaths, mut coords) = (0usize, 0usize);
    let mut check = |x: f64, y: f64| -> Result<(), String> {
        if !(x.is_finite() && y.is_finite()) || x < -0.5 || y < -0.5 || x > w + 0.5 || y > h + 0.5 {
            return Err(format!("point ({x}, {y}) is outside the page"));
        }
        coords += 1;
        Ok(())
    };
    let mut flush =
        |cmd: char, nums: &mut Vec<f64>, x: &mut f64, y: &mut f64, sx: &mut f64, sy: &mut f64| -> Result<(), String> {
            let arity = match cmd.to_ascii_lowercase() {
                'm' | 'l' => 2,
                'h' | 'v' => 1,
                'a' if allow_arcs => 7,
                'z' | ' ' => 0,
                other => return Err(format!("unexpected path command '{other}'")),
            };
            if arity == 0 {
                return if nums.is_empty() {
                    Ok(())
                } else {
                    Err(format!("stray numbers after '{cmd}'"))
                };
            }
            if nums.is_empty() || !nums.len().is_multiple_of(arity) {
                return Err(format!("command '{cmd}' has {} numbers", nums.len()));
            }
            let rel = cmd.is_ascii_lowercase();
            for (k, c) in nums.chunks(arity).enumerate() {
                let (dx, dy) = match arity {
                    2 => (c[0], c[1]),
                    7 => (c[5], c[6]),
                    _ if cmd.eq_ignore_ascii_case(&'h') => (c[0], if rel { 0.0 } else { *y }),
                    _ => (if rel { 0.0 } else { *x }, c[0]),
                };
                (*x, *y) = if rel { (*x + dx, *y + dy) } else { (dx, dy) };
                if k == 0 && cmd.eq_ignore_ascii_case(&'m') {
                    (*sx, *sy) = (*x, *y);
                }
                check(*x, *y)?;
            }
            nums.clear();
            Ok(())
        };
    let bytes = d.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c.is_ascii_alphabetic() {
            flush(cmd, &mut nums, &mut x, &mut y, &mut sx, &mut sy)?;
            cmd = c;
            if c.eq_ignore_ascii_case(&'m') {
                subpaths += 1;
            }
            if c.eq_ignore_ascii_case(&'z') {
                (x, y) = (sx, sy);
            }
            i += 1;
        } else if c == '-' || c == '.' || c.is_ascii_digit() {
            let start = i;
            i += 1;
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.' || bytes[i] == b'e') {
                i += 1;
            }
            let v: f64 = d[start..i]
                .parse()
                .map_err(|_| format!("bad number '{}'", &d[start..i]))?;
            nums.push(v);
        } else if c == ' ' || c == ',' || c == '\n' {
            i += 1;
        } else {
            return Err(format!("unexpected character '{c}' in path data"));
        }
    }
    flush(cmd, &mut nums, &mut x, &mut y, &mut sx, &mut sy)?;
    Ok((subpaths, coords))
}

pub fn validate(svg: &str, expect: &Expect) -> Result<SvgReport, Vec<String>> {
    let mut problems = Vec::new();
    let doc = match roxmltree::Document::parse(svg) {
        Ok(d) => d,
        Err(e) => return Err(vec![format!("not well-formed XML: {e}")]),
    };
    let root = doc.root_element();
    if root.tag_name().name() != "svg" || root.tag_name().namespace() != Some("http://www.w3.org/2000/svg") {
        problems.push("root element is not an SVG <svg>".into());
    }
    for (attr, want) in [("width", expect.page_width_mm), ("height", expect.page_height_mm)] {
        match length_mm(root.attribute(attr)) {
            Some(v) if (v - want).abs() <= 0.001 => {}
            other => problems.push(format!("{attr} is {other:?} mm, expected {want:.3} mm")),
        }
    }
    let vb: Vec<f64> = root
        .attribute("viewBox")
        .unwrap_or("")
        .split_whitespace()
        .filter_map(|t| t.parse().ok())
        .collect();
    let (w, h) = (expect.page_width_mm * UM_PER_MM, expect.page_height_mm * UM_PER_MM);
    if vb.len() != 4 || vb[0] != 0.0 || vb[1] != 0.0 || (vb[2] - w).abs() > 1.0 || (vb[3] - h).abs() > 1.0 {
        problems.push(format!(
            "viewBox {vb:?} does not match the page in µm (0 0 {w:.0} {h:.0})"
        ));
    }
    let mut report = SvgReport::default();
    let (mut halftone, mut marks) = (0, 0);
    for node in root.children().filter(|n| n.is_element()) {
        let id = node.attribute("id").unwrap_or("");
        if node.tag_name().name() != "path" {
            problems.push(format!(
                "unexpected <{}> element (only paths are allowed)",
                node.tag_name().name()
            ));
            continue;
        }
        let d = node.attribute("d").unwrap_or("");
        match id {
            "halftone" => {
                halftone += 1;
                if node.attribute("fill") != Some(expect.ink) {
                    problems.push(format!(
                        "halftone fill is {:?}, expected {}",
                        node.attribute("fill"),
                        expect.ink
                    ));
                }
                if !matches!(node.attribute("fill-rule"), Some("evenodd" | "nonzero")) {
                    problems.push("halftone path has no explicit fill-rule".into());
                }
                match walk_path(d, w, h, false) {
                    Ok((s, c)) => report = SvgReport { subpaths: s, coords: c },
                    Err(e) => problems.push(format!("halftone path: {e}")),
                }
            }
            "registration-marks" => {
                marks += 1;
                if node.attribute("stroke") != Some(expect.ink) || node.attribute("fill") != Some("none") {
                    problems.push("registration marks must be stroked in the ink colour with no fill".into());
                }
                if let Err(e) = walk_path(d, w, h, true) {
                    problems.push(format!("registration marks: {e}"));
                }
            }
            other => problems.push(format!("unexpected path id '{other}'")),
        }
    }
    if halftone != 1 {
        problems.push(format!("expected exactly one halftone path, found {halftone}"));
    }
    if marks != usize::from(expect.registration_marks) {
        problems.push(format!(
            "expected {} registration-mark path(s), found {marks}",
            usize::from(expect.registration_marks)
        ));
    }
    if problems.is_empty() { Ok(report) } else { Err(problems) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_walker() {
        assert_eq!(
            walk_path("M10 10l5 0 0 5 -5 0zM1 1l1 1 1 -1z", 100.0, 100.0, false),
            Ok((2, 7))
        );
        assert!(
            walk_path("M10 10l500 0z", 100.0, 100.0, false)
                .unwrap_err()
                .contains("outside")
        );
        assert!(walk_path("M10 10a1 1 0 1 0 2 0z", 100.0, 100.0, false).is_err());
        assert!(walk_path("M10 10a1 1 0 1 0 2 0z", 100.0, 100.0, true).is_ok());
        assert!(walk_path("M10 10l5", 100.0, 100.0, false).is_err());
        assert!(walk_path("M10 10h5v5H10V10z", 100.0, 100.0, false).is_ok());
    }
}
