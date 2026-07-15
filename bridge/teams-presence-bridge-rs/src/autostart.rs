use std::env;
use std::path::PathBuf;
use winreg::enums::*;
use winreg::RegKey;

const APP_NAME: &str = "TeamsPresenceBridge";

fn get_exe_path() -> Option<PathBuf> {
    env::current_exe().ok()
}

pub fn is_autostart_enabled() -> bool {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run_key = match hkcu.open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Run") {
        Ok(key) => key,
        Err(_) => return false,
    };

    let val: String = match run_key.get_value(APP_NAME) {
        Ok(v) => v,
        Err(_) => return false,
    };

    if let Some(exe_path) = get_exe_path() {
        let expected_path = format!("\"{}\"", exe_path.to_string_lossy());
        val == expected_path
    } else {
        false
    }
}

pub fn set_autostart(enabled: bool) -> Result<(), std::io::Error> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (run_key, _) = hkcu.create_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Run")?;

    if enabled {
        if let Some(exe_path) = get_exe_path() {
            let path_str = format!("\"{}\"", exe_path.to_string_lossy());
            run_key.set_value(APP_NAME, &path_str)?;
        }
    } else {
        // Ignore error if it doesn't exist
        let _ = run_key.delete_value(APP_NAME);
    }
    
    Ok(())
}
