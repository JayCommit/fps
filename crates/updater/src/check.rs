//! GitHub Releases listing for updater checks.
//!
//! GitHub `/releases/latest` excludes prereleases. Alpha and beta channels
//! must never use that endpoint. This crate always lists releases instead.

use fps_branding::Channel;

/// Path under `/repos/{owner}/{repo}` used to list candidate releases.
///
/// Never returns `/releases/latest` for alpha or beta. Stable also uses the
/// list endpoint so a single code path cannot accidentally skip prereleases.
pub fn github_releases_api_path(channel: Channel) -> &'static str {
    match channel {
        Channel::Alpha | Channel::Beta | Channel::Stable => "/releases?per_page=100",
    }
}

/// Absolute GitHub Releases API URL for listing (never `/releases/latest`).
pub fn release_list_url(owner: &str, repo: &str) -> String {
    format!("https://api.github.com/repos/{owner}/{repo}/releases?per_page=100")
}

/// Combine owner/repo with [`github_releases_api_path`].
pub fn github_releases_url(owner: &str, repo: &str, channel: Channel) -> String {
    format!(
        "https://api.github.com/repos/{owner}/{repo}{}",
        github_releases_api_path(channel)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_urls_never_use_latest() {
        for channel in [Channel::Alpha, Channel::Beta, Channel::Stable] {
            let path = github_releases_api_path(channel);
            assert!(
                !path.contains("/latest"),
                "channel {channel} path must not contain /latest: {path}"
            );
            assert_eq!(path, "/releases?per_page=100");
            let url = github_releases_url("GITHUB_OWNER", "GITHUB_REPOSITORY", channel);
            assert!(
                !url.contains("/latest"),
                "channel {channel} url must not contain /latest: {url}"
            );
        }
        let url = release_list_url("GITHUB_OWNER", "GITHUB_REPOSITORY");
        assert_eq!(
            url,
            "https://api.github.com/repos/GITHUB_OWNER/GITHUB_REPOSITORY/releases?per_page=100"
        );
        assert!(!url.contains("/latest"));
    }
}
