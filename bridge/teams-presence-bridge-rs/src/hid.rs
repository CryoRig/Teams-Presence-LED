use hidapi::{HidApi, HidDevice};

// ESP32-S3 XIAO default VID/PID, plus our custom usage page
const TARGET_VID: u16 = 0x2886;
const TARGET_PID: u16 = 0x0056;
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
}

impl HidManager {
    pub fn new() -> Self {
        let api = HidApi::new().expect("Failed to init HID API");
        Self { api, device: None }
    }

    pub fn connect(&mut self) -> bool {
        // Refresh device list
        let _ = self.api.refresh_devices();

        // Find device by usage page and VID/PID
        for info in self.api.device_list() {
            if info.usage_page() == USAGE_PAGE && info.vendor_id() == TARGET_VID && info.product_id() == TARGET_PID {
                match info.open_device(&self.api) {
                    Ok(dev) => {
                        dev.set_blocking_mode(false).ok();
                        self.device = Some(dev);
                        eprintln!("[HidManager] Connected to HID device (VID: {:04X}, PID: {:04X})", info.vendor_id(), info.product_id());
                        return true;
                    }
                    Err(e) => {
                        eprintln!("[HidManager] Failed to open: {}", e);
                    }
                }
            }
        }
        eprintln!("[HidManager] No device found with usage page 0x{:04X}", USAGE_PAGE);
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
                eprintln!("[HidManager] Write error: {}", e);
                self.device = None;
            }
        }
    }

    pub fn send_ping(&mut self) {
        self.send_report(CMD_PING, 0, 0, 0, 0);
        // Read the PONG response
        if let Some(ref dev) = self.device {
            let mut buf = [0u8; 6]; // report data (up to report size)
            match dev.read_timeout(&mut buf, 100) {
                Ok(n) if n > 0 && buf[0] == STATUS_PONG => { /* OK */ }
                Err(e) => {
                    eprintln!("[HidManager] Read error during ping: {}", e);
                    self.device = None;
                }
                _ => { /* timeout or unexpected response */ }
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

    pub fn query_firmware_version(&mut self) -> Option<(u8, u8, u8)> {
        self.send_report(CMD_VERSION, 0, 0, 0, 0);
        if let Some(ref dev) = self.device {
            let mut buf = [0u8; 6];
            match dev.read_timeout(&mut buf, 500) {
                Ok(n) if n >= 4 && buf[0] == CMD_VERSION => Some((buf[1], buf[2], buf[3])),
                _ => None,
            }
        } else {
            None
        }
    }

    pub fn enter_bootloader(&mut self) {
        self.send_report(CMD_BOOTLOADER, 0, 0, 0, 0);
        self.device = None;
    }
}
