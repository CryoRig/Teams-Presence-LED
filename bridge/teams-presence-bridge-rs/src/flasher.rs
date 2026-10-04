use std::error::Error;
use std::fs::read;
use std::path::Path;
use std::time::Duration;
use espflash::flasher::Flasher;
use espflash::target::ProgressCallbacks;
use espflash::target::Chip;
use espflash::connection::{Connection, ResetAfterOperation, ResetBeforeOperation};
use serialport::{available_ports, SerialPortInfo, SerialPortType, UsbPortInfo};

// Espressif USB-Serial/JTAG (ROM bootloader) identifiers
const ESP_USB_VID: u16 = 0x303a;
const ESP_USB_JTAG_PID: u16 = 0x1001;
const RTC_CNTL_OPTION1_REG: u32 = 0x6000_812C;
const FORCE_DOWNLOAD_BOOT_MASK: u32 = 0x1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlashStage {
    Downloading,
    WaitingForDevice,
    Connecting { port: String },
    Erasing,
    Flashing { percent: u8 },
    Verifying,
    Resetting,
    WaitingForReconnect,
    Done,
    Error(String),
}

pub enum FlashResult {
    Complete,
    VerifiedButFinalizationFailed(String),
}

fn is_esp_bootloader_port(port: &SerialPortInfo) -> bool {
    matches!(&port.port_type, SerialPortType::UsbPort(info) if info.vid == ESP_USB_VID && info.pid == ESP_USB_JTAG_PID)
}

/// Names of all currently present ESP USB-Serial/JTAG ports. Taken before entering
/// bootloader mode so the flasher can restrict itself to the port that newly appears.
pub fn list_esp_ports() -> Vec<String> {
    available_ports()
        .unwrap_or_default()
        .into_iter()
        .filter(is_esp_bootloader_port)
        .map(|p| p.port_name)
        .collect()
}

fn run_verified_firmware(port_info: &SerialPortInfo) {
    std::thread::sleep(Duration::from_millis(1500));
    let still_in_rom = available_ports().is_ok_and(|ports| ports.iter().any(|port| {
        port.port_name == port_info.port_name
            && is_esp_bootloader_port(port)
            && match (&port.port_type, &port_info.port_type) {
                (SerialPortType::UsbPort(current), SerialPortType::UsbPort(original)) => {
                    current.serial_number == original.serial_number
                }
                _ => false,
            }
    }));
    if !still_in_rom {
        return;
    }

    eprintln!("[Flasher] Verified device still on {}; clearing forced ROM boot", port_info.port_name);
    let serial_port = match serialport::new(&port_info.port_name, 115_200)
        .flow_control(serialport::FlowControl::None)
        .open_native()
    {
        Ok(port) => port,
        Err(error) => {
            eprintln!("[Flasher] Could not reopen ROM port for exit: {error}");
            return;
        }
    };
    let SerialPortType::UsbPort(usb_info) = &port_info.port_type else { return };
    let connection = Connection::new(
        serial_port,
        usb_info.clone(),
        ResetAfterOperation::HardReset,
        ResetBeforeOperation::DefaultReset,
        115_200,
    );
    let mut flasher = match Flasher::connect(connection, false, false, false, None, None) {
        Ok(flasher) => flasher,
        Err(error) => {
            eprintln!("[Flasher] Could not reconnect to ROM: {error:?}");
            return;
        }
    };
    if flasher.chip() != Chip::Esp32s3 {
        eprintln!("[Flasher] Unexpected chip during ROM exit: {}", flasher.chip());
        return;
    }
    match flasher.connection().read_reg(RTC_CNTL_OPTION1_REG) {
        Ok(value) if value & FORCE_DOWNLOAD_BOOT_MASK != 0 => {
            if let Err(error) = flasher.connection().write_reg(RTC_CNTL_OPTION1_REG, 0, Some(FORCE_DOWNLOAD_BOOT_MASK)) {
                eprintln!("[Flasher] Could not clear forced ROM boot: {error:?}");
                return;
            }
        }
        Ok(_) => {}
        Err(error) => {
            eprintln!("[Flasher] Could not read forced ROM boot state: {error:?}");
            return;
        }
    }
    if let Err(error) = flasher.connection().reset() {
        eprintln!("[Flasher] Reset after clearing forced ROM boot: {error:?}");
    }
}

struct ProgressTracker<'a, F>
where
    F: Fn(FlashStage) + Send,
{
    total_bytes: usize,
    callback: &'a F,
    verified: bool,
}

impl<'a, F> ProgressCallbacks for ProgressTracker<'a, F>
where
    F: Fn(FlashStage) + Send,
{
    fn init(&mut self, _addr: u32, total: usize) {
        self.total_bytes = total;
        (self.callback)(FlashStage::Flashing { percent: 0 });
    }

    fn update(&mut self, current: usize) {
        let percent = if self.total_bytes > 0 {
            ((current as f64 / self.total_bytes as f64) * 100.0) as u8
        } else {
            0
        };
        (self.callback)(FlashStage::Flashing { percent });
    }
    fn verifying(&mut self) {
        (self.callback)(FlashStage::Verifying);
    }

    fn finish(&mut self, skipped: bool) {
        self.verified = !skipped;
    }
}

/// Flashes `firmware_path` to the app partition of the ESP32-S3 that is in ROM bootloader mode.
/// `exclude_ports` lists ESP ports that existed *before* the bootloader command was sent;
/// only a port not in that list is accepted, so an unrelated ESP board is never flashed.
pub fn flash_firmware_esp32(
    firmware_path: &Path,
    exclude_ports: &[String],
    progress_cb: impl Fn(FlashStage) + Send,
) -> Result<FlashResult, Box<dyn Error>> {
    progress_cb(FlashStage::WaitingForDevice);

    // 1. Scan serial ports for the ESP32-S3 in bootloader mode.
    // After sending the bootloader command via HID, the device re-enumerates as
    // a USB-CDC/JTAG device. On Windows this can take several seconds, so we
    // poll with retries for up to 20 s instead of doing a single scan.
    let port_info = {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        let mut found = None;
        eprintln!("[Flasher] Waiting for ESP32-S3 (VID 0x{:04x} PID 0x{:04x}) to enumerate...", ESP_USB_VID, ESP_USB_JTAG_PID);
        while std::time::Instant::now() < deadline {
            let candidates: Vec<SerialPortInfo> = available_ports()
                .unwrap_or_default()
                .into_iter()
                .filter(is_esp_bootloader_port)
                .filter(|p| !exclude_ports.contains(&p.port_name))
                .collect();
            match candidates.len() {
                0 => {}
                1 => {
                    found = candidates.into_iter().next();
                    break;
                }
                n => {
                    let names: Vec<&str> = candidates.iter().map(|p| p.port_name.as_str()).collect();
                    return Err(format!(
                        "{} ESP devices in bootloader mode found ({}); refusing to guess which one to flash. Disconnect the others and retry.",
                        n, names.join(", ")
                    ).into());
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        match found {
            Some(p) => {
                eprintln!("[Flasher] Using {}. Waiting 1.5s for Windows driver to settle...", p.port_name);
                progress_cb(FlashStage::Connecting { port: p.port_name.clone() });
                std::thread::sleep(std::time::Duration::from_millis(1500));
                p
            },
            None => return Err("ESP32-S3 bootloader serial port not found after 20 s. Is the device connected and in bootloader mode?".into()),
        }
    };

    // 2. Open serial port
    let serial_port = serialport::new(&port_info.port_name, 115_200)
        .flow_control(serialport::FlowControl::None)
        .open_native()?;

    let usb_info = match &port_info.port_type {
        SerialPortType::UsbPort(info) => info.clone(),
        _ => UsbPortInfo {
            vid: ESP_USB_VID,
            pid: ESP_USB_JTAG_PID,
            serial_number: None,
            manufacturer: None,
            product: None,
        },
    };

    let connection = Connection::new(
        serial_port,
        usb_info,
        ResetAfterOperation::HardReset,
        // The device is already in bootloader mode (we triggered it via HID).
        // Sending a DTR/RTS reset pulse here would disrupt the stub upload,
        // causing a communication error mid-flash.
        ResetBeforeOperation::NoReset,
        115_200,
    );

    let mut flasher = Flasher::connect(
        connection,
        false, // use_stub: false for stable USB-JTAG ROM bootloader interaction
        true, // verify
        false, // skip
        None, // chip auto-detect
        None, // default baud
    )?;

    // 4. Read firmware binary
    let binary_data = read(firmware_path)?;

    // 5. Flash app binary to the application partition offset (0x10000)
    // This expects a standard PlatformIO firmware.bin (app-only), NOT a merged binary.
    // The bootloader and partition table at 0x0–0xFFFF are left untouched.
    let mut tracker = ProgressTracker {
        total_bytes: binary_data.len(),
        callback: &progress_cb,
        verified: false,
    };

    progress_cb(FlashStage::Erasing);

    let finalization_error = match flasher.write_bin_to_flash(0x10000, &binary_data, &mut tracker) {
        Ok(()) => None,
        Err(error) if tracker.verified => {
            eprintln!("[Flasher] Image verified, but ROM finalization failed: {error:#}");
            Some(error.to_string())
        }
        Err(error) => return Err(error.into()),
    };

    progress_cb(FlashStage::Resetting);
    drop(flasher);
    run_verified_firmware(&port_info);

    progress_cb(FlashStage::Done);

    Ok(match finalization_error {
        Some(error) => FlashResult::VerifiedButFinalizationFailed(error),
        None => FlashResult::Complete,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transfer_completion_does_not_imply_verification() {
        let mut tracker = ProgressTracker {
            total_bytes: 0,
            callback: &|_| {},
            verified: false,
        };

        tracker.init(0x10000, 10);
        tracker.update(10);
        tracker.verifying();
        assert!(!tracker.verified);

        tracker.finish(false);
        assert!(tracker.verified);
    }
}

