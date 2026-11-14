//! The project's version: one number in the workspace manifest, raised with
//! every feature (minor) or fix (patch), and checked on every pull request.

use std::cmp::Ordering;
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

/// The heading in the changelog that collects what is not released yet.
const UNRELEASED: &str = "## Unreleased";

/// A version of the form `major.minor.patch`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

/// Which part of the version a change raises.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    /// A change that breaks what users have: their files or how they work.
    Major,
    /// A new feature.
    Minor,
    /// A fix.
    Patch,
}

impl Part {
    pub fn named(word: &str) -> Option<Part> {
        match word {
            "major" => Some(Part::Major),
            "minor" => Some(Part::Minor),
            "patch" => Some(Part::Patch),
            _ => None,
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl Version {
    pub fn parse(text: &str) -> Option<Version> {
        let mut parts = text.trim().split('.');
        let version = Version {
            major: parts.next()?.parse().ok()?,
            minor: parts.next()?.parse().ok()?,
            patch: parts.next()?.parse().ok()?,
        };
        // A fourth part or a suffix is not something this project uses.
        parts.next().is_none().then_some(version)
    }

    /// The next version: the raised part goes up by one, the parts after it go to zero.
    pub fn raised(self, part: Part) -> Version {
        match part {
            Part::Major => Version {
                major: self.major + 1,
                minor: 0,
                patch: 0,
            },
            Part::Minor => Version {
                minor: self.minor + 1,
                patch: 0,
                ..self
            },
            Part::Patch => Version {
                patch: self.patch + 1,
                ..self
            },
        }
    }
}

/// The version in a workspace manifest: the `version = "..."` line under `[workspace.package]`.
pub fn in_manifest(manifest: &str) -> Option<Version> {
    let line = manifest.lines().nth(version_line(manifest)?)?;
    let quoted = line.split('"').nth(1)?;
    Version::parse(quoted)
}

/// The manifest with its version replaced. Everything else stays as it is.
pub fn with_version(manifest: &str, version: Version) -> Option<String> {
    let target = version_line(manifest)?;
    let lines: Vec<String> = manifest
        .lines()
        .enumerate()
        .map(|(number, line)| {
            if number == target {
                format!("version = \"{version}\"")
            } else {
                line.to_string()
            }
        })
        .collect();
    Some(lines.join("\n") + "\n")
}

/// The number of the version line inside the `[workspace.package]` table.
fn version_line(manifest: &str) -> Option<usize> {
    let mut in_table = false;
    for (number, line) in manifest.lines().enumerate() {
        let line = line.trim();
        if line.starts_with('[') {
            in_table = line == "[workspace.package]";
        } else if in_table && line.starts_with("version") && line.contains('=') {
            return Some(number);
        }
    }
    None
}

/// The changelog with what was under "Unreleased" moved under a heading for
/// `version`, and "Unreleased" left empty for what comes next.
pub fn with_release(changelog: &str, version: Version, date: &str) -> Option<String> {
    let (before, after) = changelog.split_once(UNRELEASED)?;
    let notes = after.trim_start_matches('\n');
    Some(format!(
        "{before}{UNRELEASED}\n\n## {version} - {date}\n\n{notes}"
    ))
}

/// Whether a pull request raised the version as it must: any change to the
/// crates needs a version above the one on the branch it merges into.
pub fn check(ours: Version, base: Version, crates_changed: bool) -> Result<(), String> {
    match (ours.cmp(&base), crates_changed) {
        (Ordering::Greater, _) | (Ordering::Equal, false) => Ok(()),
        (Ordering::Equal, true) => Err(format!(
            "the crates changed but the version is still {base}; run `cargo xtask bump minor` for a feature or `cargo xtask bump patch` for a fix"
        )),
        (Ordering::Less, _) => Err(format!(
            "the version {ours} is below {base} on the target branch; merge the target branch, then bump again"
        )),
    }
}

/// Today as `YYYY-MM-DD` in UTC.
pub fn today() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0);
    date_of(seconds / 86_400)
}

/// The calendar date of a day counted from 1 January 1970.
fn date_of(days_since_1970: u64) -> String {
    // Howard Hinnant's days-to-civil algorithm, for dates from 1970 on.
    let shifted = days_since_1970 + 719_468;
    let era = shifted / 146_097;
    let day_of_era = shifted % 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + u64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = "[workspace]\nmembers = [\"a\"]\n\n[workspace.package]\nversion = \"0.4.2\"\nedition = \"2024\"\n\n[workspace.lints.rust]\nversion = \"other\"\n";

    fn version(major: u64, minor: u64, patch: u64) -> Version {
        Version {
            major,
            minor,
            patch,
        }
    }

    #[test]
    fn a_version_is_three_numbers() {
        assert_eq!(Version::parse("1.20.3"), Some(version(1, 20, 3)));
        assert_eq!(Version::parse(" 0.1.0\n"), Some(version(0, 1, 0)));
        for text in ["1.2", "1.2.3.4", "1.2.x", "1.2.3-rc.1", ""] {
            assert_eq!(Version::parse(text), None, "{text:?}");
        }
    }

    #[test]
    fn raising_a_part_resets_the_parts_after_it() {
        let from = version(1, 4, 7);

        assert_eq!(from.raised(Part::Patch), version(1, 4, 8));
        assert_eq!(from.raised(Part::Minor), version(1, 5, 0));
        assert_eq!(from.raised(Part::Major), version(2, 0, 0));
    }

    #[test]
    fn versions_compare_by_number_not_by_text() {
        assert!(version(0, 10, 0) > version(0, 9, 9));
        assert!(version(1, 0, 0) > version(0, 99, 99));
    }

    #[test]
    fn the_manifest_version_is_the_one_of_the_workspace_package() {
        assert_eq!(in_manifest(MANIFEST), Some(version(0, 4, 2)));
        assert_eq!(in_manifest("[package]\nversion = \"1.0.0\"\n"), None);
    }

    #[test]
    fn replacing_the_version_touches_only_its_line() {
        let updated = with_version(MANIFEST, version(0, 5, 0)).unwrap();

        assert_eq!(updated, MANIFEST.replace("0.4.2", "0.5.0"));
        assert!(updated.contains("version = \"other\""));
    }

    #[test]
    fn a_release_takes_over_the_unreleased_notes() {
        let changelog =
            "# Changelog\n\n## Unreleased\n\n- Added tabs\n\n## 0.1.0 - 2026-01-01\n\n- First\n";

        let updated = with_release(changelog, version(0, 2, 0), "2026-11-14").unwrap();

        assert_eq!(
            updated,
            "# Changelog\n\n## Unreleased\n\n## 0.2.0 - 2026-11-14\n\n- Added tabs\n\n## 0.1.0 - 2026-01-01\n\n- First\n"
        );
    }

    #[test]
    fn a_changelog_without_the_unreleased_heading_is_refused() {
        assert_eq!(
            with_release("# Changelog\n", version(1, 0, 0), "2026-11-14"),
            None
        );
    }

    #[test]
    fn a_change_to_the_crates_needs_a_higher_version() {
        let base = version(0, 2, 0);

        assert!(check(version(0, 2, 1), base, true).is_ok());
        assert!(check(base, base, false).is_ok());
        assert!(check(base, base, true).is_err());
        assert!(check(version(0, 1, 9), base, false).is_err());
    }

    #[test]
    fn days_become_calendar_dates() {
        assert_eq!(date_of(0), "1970-01-01");
        assert_eq!(date_of(59), "1970-03-01");
        assert_eq!(date_of(19_782), "2024-02-29");
        assert_eq!(date_of(20_771), "2026-11-14");
    }
}
