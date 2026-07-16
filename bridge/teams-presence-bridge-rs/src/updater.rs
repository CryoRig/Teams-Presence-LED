use std::error::Error;
use std::fs::File;
use std::io::copy;
use std::path::PathBuf;
use serde::Deserialize;
use semver::Version;

const GITHUB_REPO: &str = "CryoRig/Teams-Presence-LED";
const FIRMWARE_ASSET_NAME: &str = "firmware.bin";
const BRIDGE_ASSET_NAME: &str = "TeamsPresenceBridge.exe";

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
    pub firmware_download_url: Option<String>,
    pub bridge_download_url: Option<String>,
    pub html_url: String,
}

#[derive(Debug, Clone)]
pub struct UpdateCheckResult {
    pub latest: ReleaseInfo,
    pub bridge_update_available: bool,
    pub firmware_update_available: bool,
}

/// Helper to parse tag name to semver, stripping leading 'v'
fn parse_tag_to_semver(tag: &str) -> Result<Version, Box<dyn Error>> {
    let clean_tag = if tag.starts_with('v') {
        &tag[1..]
    } else {
        tag
    };
    let ver = Version::parse(clean_tag)?;
    Ok(ver)
}

pub fn fetch_latest_release() -> Result<ReleaseInfo, Box<dyn Error>> {
    let url = format!("https://api.github.com/repos/{}/releases/latest", GITHUB_REPO);
    
    // GitHub API requires a User-Agent and standard headers
    let response: GithubRelease = ureq::get(&url)
        .header("User-Agent", "teams-presence-bridge-rs")
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2026-03-10")
        .call()?
        .into_body()
        .read_json()?;

    let version = parse_tag_to_semver(&response.tag_name)?;

    let mut firmware_download_url = None;
    let mut bridge_download_url = None;

    for asset in response.assets {
        if asset.name == FIRMWARE_ASSET_NAME {
            firmware_download_url = Some(asset.browser_download_url);
        } else if asset.name == BRIDGE_ASSET_NAME {
            bridge_download_url = Some(asset.browser_download_url);
        }
    }

    Ok(ReleaseInfo {
        version,
        firmware_download_url,
        bridge_download_url,
        html_url: response.html_url,
    })
}

pub fn check_updates(
    bridge_current: &Version,
    firmware_current: Option<&Version>,
    latest: &ReleaseInfo,
) -> UpdateCheckResult {
    // Bridge update is available if the latest version is greater than current bridge version
    let bridge_update_available = latest.version > *bridge_current;

    // Firmware update is available if:
    // 1. ESP is connected (we have a firmware version)
    // 2. The latest version is greater than current firmware version
    let firmware_update_available = match firmware_current {
        Some(fw_ver) => latest.version > *fw_ver,
        None => false,
    };

    UpdateCheckResult {
        latest: latest.clone(),
        bridge_update_available,
        firmware_update_available,
    }
}

pub fn download_firmware(url: &str) -> Result<PathBuf, Box<dyn Error>> {
    let file_path = std::env::temp_dir().join(format!("teams_presence_fw_{}.bin", uuid_like_random()));
    
    let response = ureq::get(url)
        .header("User-Agent", "teams-presence-bridge-rs")
        .call()?;
    
    let mut dest = File::create(&file_path)?;
    copy(&mut response.into_body().as_reader(), &mut dest)?;
    
    Ok(file_path)
}

fn uuid_like_random() -> u32 {
    use std::time::SystemTime;
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_micros() as u32)
        .unwrap_or(12345)
}
