use hidapi::{HidApi, HidDevice};
use std::time::{Duration, Instant};

// Custom VID/PID for Teams Presence Bridge
const TARGET_VID: u16 = 0x1209;
const TARGET_PID: u16 = 0x0005;
const USAGE_PAGE: u16 = 0xFF00;

// Report ID must match HID_REPORT_ID_VENDOR from ESP32 Arduino core's USBHID.h
// Enum: NONE=0, KEYBOARD=1, MOUSE=2, GAMEPAD=3, CONSUMER=4, SYSTEM=5, VENDOR=6
const HID_REPORT_ID_VENDOR: u8 = 0x06;

// Command IDs (mirror firmware)
const CMD_PING: u8         = 0x01;
const CMD_BRIGHTNESS: u8   = 0x06;
const CMD_TRANSITION: u8   = 0x07;
const CMD_BOOTLOADER: u8   = 0x09;
const CMD_VERSION: u8      = 0x0A;
const CMD_GET_CALIBRATION: u8 = 0x0B;
const CMD_PREVIEW_CALIBRATION: u8 = 0x0C;
const CMD_SAVE_CALIBRATION: u8 = 0x0D;
const CMD_CALIBRATION_TEST: u8 = 0x0E;
const CMD_GET_LED_COUNT: u8 = 0x0F;
const CMD_SET_LED_COUNT: u8 = 0x10;

// Response status codes
const STATUS_PONG: u8 = 0x01;
const STATUS_CALIBRATION: u8 = CMD_GET_CALIBRATION;
const STATUS_LED_COUNT: u8 = CMD_GET_LED_COUNT;

// CMD_CALIBRATION_TEST P1 value that exits the pattern and reloads the saved profile.
const CALIBRATION_TEST_EXIT: u8 = 0xFF;

// CMD_SET_LED_COUNT P2 selectors.
const LED_COUNT_PREVIEW: u8 = 0;
const LED_COUNT_PERSIST: u8 = 1;

const PONG_TIMEOUT: Duration = Duration::from_millis(100);
const QUERY_TIMEOUT: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CalibrationProfile {
    pub red_gain: u8,
    pub green_gain: u8,
    pub blue_gain: u8,
    pub gamma_tenths: u8,
}

impl Default for CalibrationProfile {
    fn default() -> Self {
        Self { red_gain: 100, green_gain: 100, blue_gain: 100, gamma_tenths: 10 }
    }
}

/// Gains are attenuation-only; 100 means unchanged.
pub const MAX_GAIN: u8 = 100;

impl CalibrationProfile {
    /// Scale legacy gains (>100) so the strongest channel is 100, keeping channel ratios.
    pub fn normalized(self) -> Self {
        let peak = self.red_gain.max(self.green_gain).max(self.blue_gain);
        if peak <= MAX_GAIN {
            return self;
        }
        let scale = |gain: u8| ((gain as u16 * MAX_GAIN as u16 + peak as u16 / 2) / peak as u16) as u8;
        Self {
            red_gain: scale(self.red_gain),
            green_gain: scale(self.green_gain),
            blue_gain: scale(self.blue_gain),
            gamma_tenths: self.gamma_tenths,
        }
    }
}

#[derive(Clone, Copy, Default, PartialEq)]
#[repr(u8)]
pub enum CalibrationTestPattern {
    #[default]
    White = 0,
    MidGray = 1,
}

#[derive(Default)]
pub struct CalibrationState {
    pub connected: bool,
    pub supported: Option<bool>,
    pub profile: CalibrationProfile,
    pub preview_pending: bool,
    pub save_pending: bool,
    pub test_pending: bool,
    pub cancel_pending: bool,
    pub test_pattern: CalibrationTestPattern,
    pub led_count: LedCountState,
}

/// Reply payload of CMD_GET_LED_COUNT.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LedCountInfo {
    pub active: u8,
    pub saved: u8,
    pub max: u8,
}

impl LedCountInfo {
    fn from_payload([active, saved, max, _]: [u8; 4]) -> Self {
        Self { active, saved, max }
    }
}

/// Device-stored LED chain length; `supported` is `None` until the device has been queried.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LedCountState {
    pub supported: Option<bool>,
    pub count: u8,
    pub saved: u8,
    pub max: u8,
    pub preview_pending: bool,
    pub save_pending: bool,
    pub cancel_pending: bool,
}

impl Default for LedCountState {
    fn default() -> Self {
        Self { supported: None, count: 8, saved: 8, max: 64, preview_pending: false, save_pending: false, cancel_pending: false }
    }
}

impl LedCountState {
    pub fn apply(&mut self, info: LedCountInfo) {
        self.count = info.active;
        self.saved = info.saved;
        self.max = info.max.max(1);
    }

    pub fn clear_pending(&mut self) {
        self.preview_pending = false;
        self.save_pending = false;
        self.cancel_pending = false;
    }
}

// Consecutive pings without PONG before the device is considered gone
const MAX_MISSED_PONGS: u8 = 3;

pub struct HidManager {
    api: HidApi,
    device: Option<HidDevice>,
    missing_device_logged: bool,
    missed_pongs: u8,
}

impl HidManager {
    pub fn new() -> Result<Self, hidapi::HidError> {
        let api = HidApi::new()?;
        Ok(Self {
            api,
            device: None,
            missing_device_logged: false,
            missed_pongs: 0,
        })
    }

    pub fn connect(&mut self) -> bool {
        // Refresh device list
        let _ = self.api.refresh_devices();

        // Find device by usage page and VID/PID or product name
        for info in self.api.device_list() {
            if info.usage_page() == USAGE_PAGE {
                let is_target_vid_pid = info.vendor_id() == TARGET_VID && info.product_id() == TARGET_PID;
                let is_product_match = info.product_string().is_some_and(|s| s.contains("Teams Presence Bridge"));
                
                if is_target_vid_pid || is_product_match {
                    match info.open_device(&self.api) {
                        Ok(dev) => {
                            self.device = Some(dev);
                            self.missing_device_logged = false;
                            self.missed_pongs = 0;
                            eprintln!("[HidManager] Connected to HID device (VID: {:04X}, PID: {:04X})", info.vendor_id(), info.product_id());
                            return true;
                        }
                        Err(e) => {
                            eprintln!("[HidManager] Failed to open: {}", e);
                        }
                    }
                }
            }
        }
        if !self.missing_device_logged {
            eprintln!(
                "[HidManager] No Teams Presence Bridge found (usage page 0x{:04X} with VID {:04X}/PID {:04X} or matching product string)",
                USAGE_PAGE, TARGET_VID, TARGET_PID
            );
            self.missing_device_logged = true;
        }
        false
    }

    pub fn is_connected(&self) -> bool {
        self.device.is_some()
    }

    fn send_report(&mut self, cmd: u8, p1: u8, p2: u8, p3: u8, p4: u8) {
        if let Some(ref dev) = self.device {
            // First byte is report ID (must match HID_REPORT_ID_VENDOR = 6)
            let buf = [HID_REPORT_ID_VENDOR, cmd, p1, p2, p3, p4];
            if let Err(e) = dev.write(&buf) {
                eprintln!("[HidManager] Write error on OUT endpoint: {}", e);
                self.device = None;
            }
        }
    }

    /// Drain queued input reports until one with status byte `status` arrives.
    /// Returns the 4 payload bytes of the matching report, or `None` on timeout or read error.
    /// Keepalives (`0x00`) and stale OK/PONG/ERROR reports for other commands are skipped.
    fn wait_for_status(&mut self, status: u8, timeout: Duration) -> Option<[u8; 4]> {
        let dev = self.device.as_ref()?;
        let mut buf = [0u8; 6];
        let start = Instant::now();
        while start.elapsed() < timeout {
            match dev.read_timeout(&mut buf, 10) {
                // hidapi's Windows backend prepends the report ID, so buf[0] = 0x06 and buf[1] = status code
                Ok(n) if n >= 2 && buf[0] == HID_REPORT_ID_VENDOR && buf[1] == status => {
                    let mut payload = [0u8; 4];
                    let available = n.saturating_sub(2).min(4);
                    payload[..available].copy_from_slice(&buf[2..2 + available]);
                    return Some(payload);
                }
                Err(e) => {
                    eprintln!("[HidManager] Read error: {}", e);
                    self.device = None;
                    return None;
                }
                Ok(_) => {} // timeout slice, other status, or foreign report ID
            }
        }
        None
    }

    pub fn send_ping(&mut self) {
        self.send_report(CMD_PING, 0, 0, 0, 0);
        let got_pong = self.wait_for_status(STATUS_PONG, PONG_TIMEOUT).is_some();
        if got_pong {
            self.missed_pongs = 0;
        } else if self.device.is_some() {
            self.missed_pongs += 1;
            if self.missed_pongs >= MAX_MISSED_PONGS {
                eprintln!("[HidManager] No PONG for {} consecutive pings; treating device as disconnected", self.missed_pongs);
                self.device = None;
                self.missed_pongs = 0;
            }
        }
    }

    pub fn send_color_command(&mut self, cmd_id: u8, r: u8, g: u8, b: u8) {
        self.send_report(cmd_id, r, g, b, 0);
    }

    pub fn send_brightness(&mut self, value: u8) {
        self.send_report(CMD_BRIGHTNESS, value, 0, 0, 0);
    }

    pub fn send_transition(&mut self, value: u16) {
        self.send_report(CMD_TRANSITION, (value >> 8) as u8, (value & 0xFF) as u8, 0, 0);
    }

    pub fn query_calibration(&mut self) -> Option<CalibrationProfile> {
        self.send_report(CMD_GET_CALIBRATION, 0, 0, 0, 0);
        // Older firmware answers 0xFF instead; the wait then times out and reports unsupported.
        self.wait_for_status(STATUS_CALIBRATION, QUERY_TIMEOUT)
            .map(|[red_gain, green_gain, blue_gain, gamma_tenths]| CalibrationProfile {
                red_gain,
                green_gain,
                blue_gain,
                gamma_tenths,
            })
    }

    pub fn preview_calibration(&mut self, profile: CalibrationProfile) {
        self.send_report(CMD_PREVIEW_CALIBRATION, profile.red_gain, profile.green_gain,
                         profile.blue_gain, profile.gamma_tenths);
    }

    pub fn save_calibration(&mut self) {
        self.send_report(CMD_SAVE_CALIBRATION, 0, 0, 0, 0);
    }

    pub fn start_calibration_test(&mut self, pattern: CalibrationTestPattern) {
        self.send_report(CMD_CALIBRATION_TEST, pattern as u8, 0, 0, 0);
    }

    /// Leave the test pattern and discard any previewed profile (firmware reloads NVS).
    pub fn cancel_calibration_test(&mut self) {
        self.send_report(CMD_CALIBRATION_TEST, CALIBRATION_TEST_EXIT, 0, 0, 0);
    }

    pub fn query_led_count(&mut self) -> Option<LedCountInfo> {
        self.send_report(CMD_GET_LED_COUNT, 0, 0, 0, 0);
        // Firmware before 0.7.1 answers 0xFF; the wait times out and reports unsupported.
        self.wait_for_status(STATUS_LED_COUNT, QUERY_TIMEOUT).map(LedCountInfo::from_payload)
    }

    /// Resize the active chain; `persist` also stores the count in the device's NVS.
    pub fn set_led_count(&mut self, count: u8, persist: bool) {
        let mode = if persist { LED_COUNT_PERSIST } else { LED_COUNT_PREVIEW };
        self.send_report(CMD_SET_LED_COUNT, count.max(1), mode, 0, 0);
    }

    pub fn query_firmware_version(&mut self) -> Option<(u8, u8, u8, u8)> {
        self.send_report(CMD_VERSION, 0, 0, 0, 0);
        self.wait_for_status(CMD_VERSION, QUERY_TIMEOUT)
            .map(|[major, minor, patch, variant]| (major, minor, patch, variant))
    }

    pub fn enter_bootloader(&mut self) {
        self.send_report(CMD_BOOTLOADER, 0, 0, 0, 0);
        self.device = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calibration_pattern_selectors_match_firmware_protocol() {
        assert_eq!(CalibrationTestPattern::White as u8, 0);
        assert_eq!(CalibrationTestPattern::MidGray as u8, 1);
    }

    #[test]
    fn calibration_defaults_to_white_without_pending_actions() {
        let state = CalibrationState::default();
        assert!(matches!(state.test_pattern, CalibrationTestPattern::White));
        assert!(!state.test_pending);
        assert!(!state.preview_pending);
        assert!(!state.save_pending);
        assert!(!state.cancel_pending);
    }

    #[test]
    fn legacy_gains_normalize_to_attenuation_only_keeping_ratios() {
        let legacy = CalibrationProfile { red_gain: 150, green_gain: 100, blue_gain: 100, gamma_tenths: 12 };
        assert_eq!(legacy.normalized(), CalibrationProfile { red_gain: 100, green_gain: 67, blue_gain: 67, gamma_tenths: 12 });
        let extreme = CalibrationProfile { red_gain: 200, green_gain: 200, blue_gain: 50, gamma_tenths: 10 };
        assert_eq!(extreme.normalized(), CalibrationProfile { red_gain: 100, green_gain: 100, blue_gain: 25, gamma_tenths: 10 });
        let in_range = CalibrationProfile { red_gain: 80, green_gain: 100, blue_gain: 90, gamma_tenths: 10 };
        assert_eq!(in_range.normalized(), in_range);
    }

    #[test]
    fn led_count_opcodes_match_firmware_protocol() {
        assert_eq!(CMD_GET_LED_COUNT, 0x0F);
        assert_eq!(CMD_SET_LED_COUNT, 0x10);
        assert_eq!(STATUS_LED_COUNT, CMD_GET_LED_COUNT);
        assert_eq!((LED_COUNT_PREVIEW, LED_COUNT_PERSIST), (0, 1));
    }

    #[test]
    fn led_count_reply_payload_maps_active_saved_max() {
        let info = LedCountInfo::from_payload([3, 8, 64, 0]);
        assert_eq!(info, LedCountInfo { active: 3, saved: 8, max: 64 });
        let mut state = LedCountState::default();
        assert_eq!(state.supported, None);
        assert!(!state.preview_pending && !state.save_pending && !state.cancel_pending);
        state.apply(info);
        assert_eq!((state.count, state.saved, state.max), (3, 8, 64));
        state.apply(LedCountInfo { active: 1, saved: 1, max: 0 });
        assert_eq!(state.max, 1);
    }
}
