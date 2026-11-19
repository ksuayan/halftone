//! Configuration files: strict JSON with field-path errors, and JSON Schema.

use crate::Error;
use halftone_types::Config;
use std::path::Path;

/// Parse a config; errors name the failing field (e.g. `screen.lpi: invalid type`).
pub fn from_json(text: &str, path: &Path) -> Result<Config, Error> {
    let de = &mut serde_json::Deserializer::from_str(text);
    serde_path_to_error::deserialize(de).map_err(|e| {
        let at = e.path().to_string();
        let message = if at == "." {
            e.inner().to_string()
        } else {
            format!("{at}: {}", e.inner())
        };
        Error::Json {
            path: path.to_path_buf(),
            message,
        }
    })
}

pub fn load(path: &Path) -> Result<Config, Error> {
    let text = std::fs::read_to_string(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    from_json(&text, path)
}

pub fn to_json(config: &Config) -> String {
    let mut s = serde_json::to_string_pretty(config).expect("config serializes");
    s.push('\n');
    s
}

/// JSON Schema for the configuration file.
pub fn schema() -> String {
    let mut s = serde_json::to_string_pretty(&schemars::schema_for!(Config)).expect("schema serializes");
    s.push('\n');
    s
}
