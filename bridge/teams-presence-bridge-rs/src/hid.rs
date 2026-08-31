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

// Response status codes
const STATUS_PONG: u8 = 0x01;

pub struct HidManager {
    api: HidApi,
    device: Option<HidDevice>,
    missing_device_logged: bool,
}

impl HidManager {
    pub fn new() -> Result<Self, hidapi::HidError> {
        let api = HidApi::new()?;
        Ok(Self {
            api,
            device: None,
            missing_device_logged: false,
        })
    }

    pub fn connect(&mut self) -> bool {
        // Refresh device list
        let _ = self.api.refresh_devices();

        // Find device by usage page and VID/PID
        for info in self.api.device_list() {
            if info.usage_page() == USAGE_PAGE && info.vendor_id() == TARGET_VID && info.product_id() == TARGET_PID {
                match info.open_device(&self.api) {
                    Ok(dev) => {
                        self.device = Some(dev);
                        self.missing_device_logged = false;
                        eprintln!("[HidManager] Connected to HID device (VID: {:04X}, PID: {:04X})", info.vendor_id(), info.product_id());
                        return true;
                    }
                    Err(e) => {
                        eprintln!("[HidManager] Failed to open: {}", e);
                    }
                }
            }
        }
        if !self.missing_device_logged {
            eprintln!("[HidManager] No device found with usage page 0x{:04X}", USAGE_PAGE);
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
                    Ok(n) if n > 1 => {
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
                    Ok(_) => {} // timeout or small n
                }
            }
        }
        (got_pong, got_version)
    }

    pub fn send_ping(&mut self) {
        self.send_report(CMD_PING, 0, 0, 0, 0);
        self.drain_reads(true, false);
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

    pub fn query_firmware_version(&mut self) -> Option<(u8, u8, u8, u8)> {
        self.send_report(CMD_VERSION, 0, 0, 0, 0);
        self.drain_reads(false, true).1
    }

    pub fn enter_bootloader(&mut self) {
        self.send_report(CMD_BOOTLOADER, 0, 0, 0, 0);
        self.device = None;
    }
}
