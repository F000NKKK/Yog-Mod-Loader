//! Versions: ordered and matched the one way every part of Yog does it.

use std::cmp::Ordering;

/// Versions as semver where they are one (`21.0.5+11` too), filling a
/// missing minor or patch (`1.99`, `26.2`); else as text.
pub fn compare(a: &str, b: &str) -> Ordering {
    match (lenient(a), lenient(b)) {
        (Some(x), Some(y)) => x.cmp_precedence(&y),
        _ => a.cmp(b),
    }
}

/// Whether `version` meets `requirement` (semver: `>=0.47.0, <0.48`).
pub fn meets(version: &str, requirement: &str) -> bool {
    match (lenient(version), semver::VersionReq::parse(requirement)) {
        (Some(v), Ok(r)) => r.matches(&v),
        _ => false,
    }
}

/// `v` as semver, a leading `v` dropped and a missing minor or patch filled.
fn lenient(v: &str) -> Option<semver::Version> {
    let v = v.trim_start_matches('v');
    semver::Version::parse(v).ok().or_else(|| {
        let core = v.split(['-', '+']).next().unwrap_or(v);
        let parts = core.split('.').count();
        let filled = format!(
            "{core}{}{}",
            ".0".repeat(3usize.saturating_sub(parts)),
            &v[core.len()..]
        );
        semver::Version::parse(&filled).ok()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_order_as_semver() {
        assert_eq!(compare("1.20.1", "1.21.1"), Ordering::Less);
        assert_eq!(compare("26.2", "1.21.1"), Ordering::Greater);
        assert_eq!(compare("0.47.1", "0.47.0"), Ordering::Greater);
        assert_eq!(compare("21.0.5+11", "21.0.10+7"), Ordering::Less);
        assert_eq!(compare("1.99", "1.100.0"), Ordering::Less);
        assert_eq!(compare("v0.6.1", "0.6.1"), Ordering::Equal);
    }

    #[test]
    fn requirements_match() {
        assert!(meets("1.99", ">=1.99"));
        assert!(meets("0.47.1", ">=0.47.0, <0.48"));
        assert!(!meets("0.46.2", ">=0.47.0"));
    }
}
