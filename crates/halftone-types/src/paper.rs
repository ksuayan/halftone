//! Named paper sizes (portrait, millimetres). Add sizes here; nothing else changes.

pub struct Paper {
    pub name: &'static str,
    pub width_mm: f64,
    pub height_mm: f64,
}

pub const PAPERS: &[Paper] = &[
    Paper {
        name: "letter",
        width_mm: 215.9,
        height_mm: 279.4,
    },
    Paper {
        name: "legal",
        width_mm: 215.9,
        height_mm: 355.6,
    },
    Paper {
        name: "tabloid",
        width_mm: 279.4,
        height_mm: 431.8,
    },
    Paper {
        name: "9x12",
        width_mm: 228.6,
        height_mm: 304.8,
    },
    Paper {
        name: "a5",
        width_mm: 148.0,
        height_mm: 210.0,
    },
    Paper {
        name: "a4",
        width_mm: 210.0,
        height_mm: 297.0,
    },
    Paper {
        name: "a3",
        width_mm: 297.0,
        height_mm: 420.0,
    },
];

/// Case-insensitive lookup; `None` lists nothing, callers use [`names`] for help text.
pub fn find(name: &str) -> Option<&'static Paper> {
    let n = name.to_ascii_lowercase();
    PAPERS.iter().find(|p| p.name == n)
}

pub fn names() -> String {
    PAPERS.iter().map(|p| p.name).collect::<Vec<_>>().join(", ")
}

#[cfg(test)]
mod tests {
    #[test]
    fn lookup() {
        assert_eq!(super::find("A3").unwrap().height_mm, 420.0);
        assert!(super::find("b5").is_none());
        assert!(super::names().contains("9x12"));
    }
}
