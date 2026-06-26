//! Global Portier settings at `~/.config/portier/config.toml`.
//!
//! Optional. When the file is missing or malformed, sensible defaults are used
//! (search range 3000–7000, sequential allocation, skip well-known ports).
//!
//! ```toml
//! [ranges]
//! start = 3000
//! end   = 7000
//!
//! [preferences]
//! prefer_consecutive = false
//! avoid_well_known   = true
//! ```

use std::ops::Range;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

fn default_start() -> u16 {
    3000
}
fn default_end() -> u16 {
    7000
}
fn default_true() -> bool {
    true
}

/// The free-port search window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ranges {
    #[serde(default = "default_start")]
    pub start: u16,
    #[serde(default = "default_end")]
    pub end: u16,
}

impl Default for Ranges {
    fn default() -> Self {
        Self {
            start: default_start(),
            end: default_end(),
        }
    }
}

/// Allocation preferences.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preferences {
    /// Pack a project's services into consecutive ports when possible.
    #[serde(default)]
    pub prefer_consecutive: bool,
    /// Never allocate a well-known port (< 1024).
    #[serde(default = "default_true")]
    pub avoid_well_known: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            prefer_consecutive: false,
            avoid_well_known: true,
        }
    }
}

/// Top-level global settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub ranges: Ranges,
    #[serde(default)]
    pub preferences: Preferences,
}

impl Settings {
    /// On-disk path for the settings file.
    pub fn path() -> Option<PathBuf> {
        let home = std::env::var("HOME")
            .ok()
            .or_else(|| std::env::var("USERPROFILE").ok())?;
        Some(
            PathBuf::from(home)
                .join(".config")
                .join("portier")
                .join("config.toml"),
        )
    }

    /// Load settings, falling back to defaults if the file is absent or invalid.
    pub fn load() -> Settings {
        let Some(path) = Self::path() else {
            return Settings::default();
        };
        let Ok(data) = std::fs::read_to_string(&path) else {
            return Settings::default();
        };
        toml::from_str(&data).unwrap_or_default()
    }

    /// The effective free-port search range, honoring `avoid_well_known`.
    pub fn port_range(&self) -> Range<u16> {
        let mut start = self.ranges.start;
        if self.preferences.avoid_well_known && start < 1024 {
            start = 1024;
        }
        let end = self.ranges.end.max(start.saturating_add(1));
        start..end
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_defaults() {
        let s = Settings::default();
        assert_eq!(s.port_range(), 3000..7000);
        assert!(!s.preferences.prefer_consecutive);
        assert!(s.preferences.avoid_well_known);
    }

    #[test]
    fn test_partial_toml_fills_defaults() {
        // Only `start` given — `end` should default to 7000.
        let s: Settings = toml::from_str("[ranges]\nstart = 4000\n").unwrap();
        assert_eq!(s.port_range(), 4000..7000);
    }

    #[test]
    fn test_avoid_well_known_bumps_start() {
        let s: Settings = toml::from_str("[ranges]\nstart = 80\n").unwrap();
        // avoid_well_known defaults to true, so start is lifted to 1024.
        assert_eq!(s.port_range().start, 1024);
    }

    #[test]
    fn test_allow_well_known_when_disabled() {
        let s: Settings =
            toml::from_str("[ranges]\nstart = 80\n[preferences]\navoid_well_known = false\n")
                .unwrap();
        assert_eq!(s.port_range().start, 80);
    }

    #[test]
    fn test_prefer_consecutive_parsed() {
        let s: Settings =
            toml::from_str("[preferences]\nprefer_consecutive = true\n").unwrap();
        assert!(s.preferences.prefer_consecutive);
    }
}
