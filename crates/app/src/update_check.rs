//! "Check for updates" on the About page: asks GitHub for the newest release and compares
//! it with the running version. It runs only when the button is pressed.

use std::time::Duration;

use semver::Version;

/// Where GitHub names the newest published release (drafts and pre-releases are left out).
const LATEST_RELEASE: &str = "https://api.github.com/repos/bartekmp/remotething/releases/latest";
const TIMEOUT: Duration = Duration::from_secs(10);

/// What the About page shows beside the button.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Status {
    #[default]
    Idle,
    Checking,
    UpToDate,
    /// The version number of a newer release.
    Available(String),
    Failed(String),
}

/// Asks GitHub which release is the newest, without blocking the interface.
pub async fn check() -> Result<String, String> {
    tokio::task::spawn_blocking(latest_version)
        .await
        .map_err(|error| error.to_string())?
}

fn latest_version() -> Result<String, String> {
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .user_agent(concat!("remotething/", env!("CARGO_PKG_VERSION")))
        .build()
        .new_agent();
    let text = agent
        .get(LATEST_RELEASE)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|error| error.to_string())?
        .body_mut()
        .read_to_string()
        .map_err(|error| error.to_string())?;
    let body: serde_json::Value = serde_json::from_str(&text).map_err(|error| error.to_string())?;
    let tag = body["tag_name"]
        .as_str()
        .ok_or("the answer has no version")?;
    Ok(tag.trim_start_matches('v').to_string())
}

/// The status once `latest` (a version number) is known, as seen from `current`.
pub fn compare(current: &str, latest: &str) -> Status {
    match (Version::parse(current), Version::parse(latest)) {
        (Ok(current), Ok(latest)) if latest > current => Status::Available(latest.to_string()),
        (Ok(_), Ok(_)) => Status::UpToDate,
        _ => Status::Failed(format!("unreadable version: {latest}")),
    }
}

/// The status after the answer of [`check`] arrived.
pub fn finished(answer: Result<String, String>) -> Status {
    match answer {
        Ok(latest) => compare(env!("CARGO_PKG_VERSION"), &latest),
        Err(reason) => Status::Failed(reason),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_newer_release_is_available() {
        assert_eq!(
            compare("0.85.1", "0.86.0"),
            Status::Available("0.86.0".into())
        );
        assert_eq!(
            compare("0.9.0", "0.10.0"),
            Status::Available("0.10.0".into())
        );
    }

    #[test]
    fn the_same_or_an_older_release_is_up_to_date() {
        assert_eq!(compare("0.85.1", "0.85.1"), Status::UpToDate);
        assert_eq!(compare("0.86.0", "0.85.1"), Status::UpToDate);
    }

    #[test]
    fn an_unreadable_version_is_a_failure() {
        assert!(matches!(compare("0.85.1", "nightly"), Status::Failed(_)));
    }

    #[test]
    fn a_failed_request_keeps_its_reason() {
        assert_eq!(
            finished(Err("timeout".into())),
            Status::Failed("timeout".into())
        );
    }

    /// Needs the internet: `cargo test -p remotething update_check -- --ignored`.
    #[test]
    #[ignore = "asks GitHub"]
    fn github_names_a_version() {
        let latest = latest_version().unwrap();

        assert!(Version::parse(&latest).is_ok(), "{latest}");
    }
}
