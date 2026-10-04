use std::error::Error;
use std::io::Read;
use std::io::copy;
use std::path::PathBuf;
use serde::Deserialize;
use semver::Version;
use sha2::{Digest, Sha256};
use tempfile::Builder;
use ureq::tls::{RootCerts, TlsConfig};

const GITHUB_REPO: &str = "CryoRig/Teams-Presence-LED";
pub const GITHUB_REPO_URL: &str = "https://github.com/CryoRig/Teams-Presence-LED";

#[derive(Debug, Clone, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Debug, Clone, Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Clone)]
pub struct ReleaseInfo {
    pub version: Version,
    pub firmware_version: Version,
    pub firmware_download_url: Option<String>,
    pub firmware_sha256sums_url: Option<String>,
    pub html_url: String,
}

#[derive(Debug, Clone)]
pub struct UpdateCheckResult {
    pub bridge_update_available: bool,
    pub firmware_update_available: bool,
}

/// Helper to parse tag name to semver, stripping leading 'v'
fn parse_tag_to_semver(tag: &str) -> Result<Version, Box<dyn Error>> {
    let clean_tag = tag.strip_prefix('v').unwrap_or(tag);
    let ver = Version::parse(clean_tag)?;
    Ok(ver)
}

/// Build an ureq Agent that validates TLS certificates against the platform's
/// certificate store (e.g. Windows Certificate Store). This ensures that
/// corporate CA certificates installed on the host are trusted, which is
/// required in environments with TLS-inspecting corporate firewalls.
fn build_agent() -> ureq::Agent {
    let tls_config = TlsConfig::builder()
        .root_certs(RootCerts::PlatformVerifier)
        .build();

    ureq::Agent::config_builder()
        .tls_config(tls_config)
        .build()
        .new_agent()
}

pub fn fetch_latest_release() -> Result<ReleaseInfo, Box<dyn Error>> {
    let url = format!("https://api.github.com/repos/{}/releases/latest", GITHUB_REPO);
    
    // GitHub API requires a User-Agent and standard headers
    let response: GithubRelease = build_agent()
        .get(&url)
        .header("User-Agent", "teams-presence-bridge-rs")
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2026-03-10")
        .call()?
        .into_body()
        .read_json()?;

    let version = parse_tag_to_semver(&response.tag_name)?;

    let mut firmware_download_url = None;
    let mut firmware_sha256sums_url = None;
    let mut manifest_url = None;

    for asset in response.assets {
        if asset.name == "seeed_xiao_esp32s3.bin" || asset.name == "firmware.bin" {
            firmware_download_url = Some(asset.browser_download_url);
        } else if asset.name.eq_ignore_ascii_case("SHA256SUMS.txt")
            || asset.name.eq_ignore_ascii_case("SHA256SUMS")
            || asset.name.ends_with(".sha256")
        {
            firmware_sha256sums_url = Some(asset.browser_download_url);
        } else if asset.name == "manifest.json" {
            manifest_url = Some(asset.browser_download_url);
        }
    }

    let mut br_ver = version.clone();
    let mut fw_ver = version.clone();
    if let Some(m_url) = manifest_url
        && let Ok(m_resp) = build_agent().get(&m_url).call()
            && let Ok(json) = m_resp.into_body().read_json::<serde_json::Value>() {
                if let Some(f) = json.get("firmware").and_then(|v| v.as_str())
                    && let Ok(v) = Version::parse(f.trim_start_matches('v')) { fw_ver = v; }
                if let Some(b) = json.get("bridge").and_then(|v| v.as_str())
                    && let Ok(v) = Version::parse(b.trim_start_matches('v')) { br_ver = v; }
            }

    Ok(ReleaseInfo {
        version: br_ver,
        firmware_version: fw_ver,
        firmware_download_url,
        firmware_sha256sums_url,
        html_url: response.html_url,
    })
}

pub fn check_updates(
    bridge_current: &Version,
    firmware_current: Option<&(Version, u8)>,
    latest: &ReleaseInfo,
) -> UpdateCheckResult {
    // Bridge update is available if the latest version is greater than current bridge version
    let bridge_update_available = latest.version > *bridge_current;

    // Firmware update is available if:
    // 1. ESP is connected (we have a firmware version)
    // 2. The latest version is greater than current firmware version
    let firmware_update_available = match firmware_current {
        Some((fw_ver, 1)) => latest.firmware_version > *fw_ver,
        None => false,
        _ => false,
    };

    UpdateCheckResult {
        bridge_update_available,
        firmware_update_available,
    }
}

#[cfg(test)]
mod variant_tests {
    use super::*;

    #[test]
    fn only_xiao_esp32s3_receives_firmware_update() {
        let release = ReleaseInfo {
            version: Version::new(1, 0, 0),
            firmware_version: Version::new(1, 0, 0),
            firmware_download_url: None,
            firmware_sha256sums_url: None,
            html_url: String::new(),
        };
        let old_firmware = Version::new(0, 6, 2);
        assert!(check_updates(&release.version, Some(&(old_firmware.clone(), 1)), &release).firmware_update_available);
        assert!(!check_updates(&release.version, Some(&(old_firmware, 2)), &release).firmware_update_available);
    }
}

fn asset_name_from_url(url: &str) -> Result<String, Box<dyn Error>> {
    let no_query = url.split('?').next().unwrap_or(url);
    let name = no_query
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .ok_or("Unable to determine asset name from URL")?;
    Ok(name.to_string())
}

fn verify_firmware_magic(bytes: &[u8]) -> Result<(), Box<dyn Error>> {
    if bytes.first().copied() != Some(0xE9) {
        return Err("Downloaded .bin does not look like a valid ESP image (missing 0xE9 magic)".into());
    }
    Ok(())
}

fn verify_firmware_sha256(
    firmware_url: &str,
    sha256sums_url: &str,
    bytes: &[u8],
) -> Result<(), Box<dyn Error>> {
    let checksums = build_agent()
        .get(sha256sums_url)
        .header("User-Agent", "teams-presence-bridge-rs")
        .call()?
        .into_body()
        .read_to_string()?;

    let asset_name = asset_name_from_url(firmware_url)?;
    let expected_hash = checksums
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let hash = parts.next()?;
            let file = parts.next()?.trim_start_matches('*');
            if file == asset_name {
                Some(hash.to_string())
            } else {
                None
            }
        })
        .next()
        .ok_or_else(|| format!("No SHA256 entry found for '{}' in SHA256SUMS", asset_name))?;

    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let actual_hash = format!("{:x}", hasher.finalize());

    if !actual_hash.eq_ignore_ascii_case(&expected_hash) {
        return Err(format!(
            "SHA256 mismatch for '{}': expected {}, got {}",
            asset_name, expected_hash, actual_hash
        )
        .into());
    }

    Ok(())
}

pub fn download_firmware(url: &str, sha256sums_url: Option<&str>) -> Result<PathBuf, Box<dyn Error>> {
    let response = build_agent()
        .get(url)
        .header("User-Agent", "teams-presence-bridge-rs")
        .call()?;

    let mut bytes = Vec::new();
    response.into_body().as_reader().read_to_end(&mut bytes)?;

    verify_firmware_magic(&bytes)?;

    let sums_url = sha256sums_url.ok_or("Release is missing SHA256SUMS asset; refusing unverified firmware download")?;
    verify_firmware_sha256(url, sums_url, &bytes)?;

    let mut tmp = Builder::new()
        .prefix("teams_presence_fw_")
        .suffix(".bin")
        .tempfile_in(std::env::temp_dir())?;

    copy(&mut std::io::Cursor::new(bytes), tmp.as_file_mut())?;

    let (_file, file_path) = tmp.keep()?;
    Ok(file_path)
}
