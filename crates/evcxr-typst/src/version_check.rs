// Copyright 2026 The evcxr-typst Authors.
// Licensed under MIT OR Apache-2.0.

//! CLI version enforcement via the `<evcxr-min-cli>` marker (D-019).
//!
//! The Typst package may declare `min-cli: "X.Y.Z"` on its `setup()` call,
//! which causes the package to emit `[#metadata("X.Y.Z")<evcxr-min-cli>]`.
//! This module reads that marker (queried in `discovery.rs`) and compares it
//! against the running CLI version. If the running CLI is older, we return an
//! error before any snippet is evaluated — fail fast with a useful message.

/// Parsed triple from a `"X.Y.Z"` semver string (pre-release tags ignored
/// per scope: D-019 says pre-release is out of scope for v0).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Version(u64, u64, u64);

impl Version {
    fn parse(s: &str) -> Option<Self> {
        // Trim a leading "v" if present (e.g. "v0.1.0").
        let s = s.trim().trim_start_matches('v');
        // Take only the numeric part before any pre-release suffix.
        let core = s.split(['-', '+']).next().unwrap_or(s);
        let mut parts = core.splitn(3, '.');
        let major = parts.next()?.parse::<u64>().ok()?;
        let minor = parts.next()?.parse::<u64>().ok()?;
        let patch = parts.next().unwrap_or("0").parse::<u64>().ok()?;
        Some(Self(major, minor, patch))
    }
}

/// Check whether the running CLI satisfies the `min-cli` requirement declared
/// by the document.
///
/// `min_cli_value` is the raw string value from the `<evcxr-min-cli>` marker
/// (may be `"X.Y.Z"` or any unparseable value).
///
/// Returns:
/// - `Ok(())` when `min_cli_value` is absent (`None`), unparseable (silent
///   per D-019 — a doc bug, not a CLI bug), or when the running CLI is at
///   or above the requirement.
/// - `Err((required, actual))` with the version strings when the CLI is too
///   old.
pub(crate) fn check(min_cli_value: Option<&str>) -> Result<(), (String, String)> {
    let required_str = match min_cli_value {
        None => return Ok(()),
        Some(s) => s,
    };
    let required = match Version::parse(required_str) {
        // Unparseable → silently proceed (D-019: "unparseable is a doc bug").
        None => return Ok(()),
        Some(v) => v,
    };
    let actual_str = env!("CARGO_PKG_VERSION");
    let actual = match Version::parse(actual_str) {
        None => return Ok(()),
        Some(v) => v,
    };
    if actual < required {
        Err((required_str.to_owned(), actual_str.to_owned()))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_parse_roundtrip() {
        assert_eq!(Version::parse("0.1.0"), Some(Version(0, 1, 0)));
        assert_eq!(Version::parse("1.2.3"), Some(Version(1, 2, 3)));
        assert_eq!(Version::parse("v2.0.0"), Some(Version(2, 0, 0)));
        assert_eq!(Version::parse("1.0.0-alpha"), Some(Version(1, 0, 0)));
    }

    #[test]
    fn version_ordering() {
        assert!(Version(0, 1, 0) > Version(0, 0, 9));
        assert!(Version(1, 0, 0) > Version(0, 9, 9));
        assert!(Version(0, 1, 0) == Version(0, 1, 0));
        assert!(Version(0, 2, 0) > Version(0, 1, 99));
    }

    #[test]
    fn check_absent_min_cli() {
        assert!(check(None).is_ok());
    }

    #[test]
    fn check_unparseable_min_cli() {
        assert!(check(Some("not-a-version")).is_ok());
        assert!(check(Some("")).is_ok());
    }

    #[test]
    fn check_cli_older_than_required() {
        // "999.0.0" will always be newer than any real CARGO_PKG_VERSION.
        let result = check(Some("999.0.0"));
        assert!(result.is_err());
        let (required, actual) = result.unwrap_err();
        assert_eq!(required, "999.0.0");
        assert_eq!(actual, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn check_cli_newer_than_required() {
        // "0.0.0" is always older than any real CARGO_PKG_VERSION (which is
        // at least 0.1.0 per the T-I08 version bump).
        assert!(check(Some("0.0.0")).is_ok());
    }

    #[test]
    fn check_equal_versions() {
        let current = env!("CARGO_PKG_VERSION");
        assert!(check(Some(current)).is_ok());
    }
}
