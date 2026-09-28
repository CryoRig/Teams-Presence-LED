use hidapi::{HidApi, HidDevice};

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

// Response status codes
const STATUS_PONG: u8 = 0x01;
const STATUS_CALIBRATION: u8 = CMD_GET_CALIBRATION;

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

#[derive(Default)]
pub struct CalibrationState {
    pub connected: bool,
    pub supported: Option<bool>,
    pub profile: CalibrationProfile,
    pub preview_pending: bool,
    pub save_pending: bool,
    pub test_pending: bool,
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

    fn drain_reads(&mut self, wait_for_pong: bool, wait_for_version: bool) -> (bool, Option<(u8, u8, u8, u8)>) {
        let mut got_pong = false;
        let mut got_version = None;
        if let Some(ref dev) = self.device {
            let mut buf = [0u8; 6];
            let start = std::time::Instant::now();
            let timeout = if wait_for_version { 500 } else { 100 };
            
            while start.elapsed().as_millis() < timeout {
                match dev.read_timeout(&mut buf, 10) {
                    // hidapi's Windows backend prepends the report ID, so buf[0] = 0x06 and buf[1] = status code
                    Ok(n) if n > 1 && buf[0] == HID_REPORT_ID_VENDOR => {
                        if buf[1] == STATUS_PONG {
                            got_pong = true;
                        } else if buf[1] == CMD_VERSION && n >= 6 {
                            got_version = Some((buf[2], buf[3], buf[4], buf[5]));
                        }
                        if (!wait_for_pong || got_pong) && (!wait_for_version || got_version.is_some()) {
                            break;
                        }
                    }
                    Err(e) => {
                        eprintln!("[HidManager] Read error: {}", e);
                        self.device = None;
                        break;
                    }
                    Ok(_) => {} // timeout, small n, or foreign report ID
                }
            }
        }
        (got_pong, got_version)
    }

    pub fn send_ping(&mut self) {
        self.send_report(CMD_PING, 0, 0, 0, 0);
        let (got_pong, _) = self.drain_reads(true, false);
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
        let device = self.device.as_ref()?;
        let mut buf = [0u8; 6];
        let start = std::time::Instant::now();
        while start.elapsed().as_millis() < 500 {
            match device.read_timeout(&mut buf, 20) {
                Ok(n) if n >= 6 && buf[0] == HID_REPORT_ID_VENDOR => {
                    if buf[1] == STATUS_CALIBRATION {
                        return Some(CalibrationProfile {
                            red_gain: buf[2],
                            green_gain: buf[3],
                            blue_gain: buf[4],
                            gamma_tenths: buf[5],
                        });
                    }
                    if buf[1] == 0xFF {
                        return None;
                    }
                }
                Err(e) => {
                    eprintln!("[HidManager] Calibration read error: {}", e);
                    self.device = None;
                    return None;
                }
                Ok(_) => {}
            }
        }
        None
    }

    pub fn preview_calibration(&mut self, profile: CalibrationProfile) {
        self.send_report(CMD_PREVIEW_CALIBRATION, profile.red_gain, profile.green_gain,
                         profile.blue_gain, profile.gamma_tenths);
    }

    pub fn save_calibration(&mut self) {
        self.send_report(CMD_SAVE_CALIBRATION, 0, 0, 0, 0);
    }

    pub fn start_calibration_test(&mut self) {
        self.send_report(CMD_CALIBRATION_TEST, 0, 0, 0, 0);
    }

    pub fn query_firmware_version(&mut self) -> Option<(u8, u8, u8, u8)> {
        self.send_report(CMD_VERSION, 0, 0, 0, 0);
        self.drain_reads(false, true).1
    }

    pub fn enter_bootloader(&mut self) {
        self.send_report(CMD_BOOTLOADER, 0, 0, 0, 0);
        self.device = None;
    }
}
