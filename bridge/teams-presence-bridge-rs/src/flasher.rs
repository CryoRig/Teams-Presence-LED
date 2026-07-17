use std::error::Error;
use std::fs::read;
use std::path::Path;
use espflash::flasher::Flasher;
use espflash::target::ProgressCallbacks;
use espflash::connection::{Connection, ResetAfterOperation, ResetBeforeOperation};
use serialport::{available_ports, SerialPortType, UsbPortInfo};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlashStage {
    Connecting,
    Erasing,
    Flashing { percent: u8 },
    Verifying,
    Resetting,
    Done,
    Error(String),
}

struct ProgressTracker<'a, F>
where
    F: Fn(FlashStage) + Send,
{
    total_bytes: usize,
    callback: &'a F,
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

    fn finish(&mut self, _skipped: bool) {
    }
}

pub fn flash_firmware(
    firmware_path: &Path,
    progress_cb: impl Fn(FlashStage) + Send,
) -> Result<(), Box<dyn Error>> {
    progress_cb(FlashStage::Connecting);

    // 1. Scan serial ports for the ESP32-S3 in bootloader mode (VID: 0x303a).
    // After sending the bootloader command via HID, the device re-enumerates as
    // a USB-CDC/JTAG device. On Windows this can take several seconds, so we
    // poll with retries for up to 20 s instead of doing a single scan.
    let port_info = {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        let mut found = None;
        eprintln!("[Flasher] Waiting for ESP32-S3 (VID: 0x303a) to enumerate...");
        while std::time::Instant::now() < deadline {
            if let Ok(ports) = available_ports() {
                for port in ports {
                    if let SerialPortType::UsbPort(ref info) = port.port_type {
                        eprintln!("[Flasher] Found USB port: {} (VID: 0x{:04x}, PID: 0x{:04x})", port.port_name, info.vid, info.pid);
                        if info.vid == 0x303a {
                            found = Some(port);
                            break;
                        }
                    }
                }
            }
            if found.is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        match found {
            Some(p) => {
                eprintln!("[Flasher] Match found! Waiting 1.5s for Windows driver to settle...");
                std::thread::sleep(std::time::Duration::from_millis(1500));
                p
            },
            None => return Err("ESP32-S3 serial port (VID 0x303a) not found after 20 s. Is the device connected and in bootloader mode?".into()),
        }
    };

    // 2. Open serial port
    let serial_port = serialport::new(&port_info.port_name, 115_200)
        .flow_control(serialport::FlowControl::None)
        .open_native()?;

    let usb_info = match port_info.port_type {
        SerialPortType::UsbPort(info) => info,
        _ => UsbPortInfo {
            vid: 0x303a,
            pid: 0x1001,
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
    };

    progress_cb(FlashStage::Erasing);

    flasher.write_bin_to_flash(0x10000, &binary_data, &mut tracker)?;

    progress_cb(FlashStage::Resetting);
    
    // Dropping flasher performs the ResetAfterOperation
    drop(flasher);

    progress_cb(FlashStage::Done);
    
    // 6. Clean up firmware file
    let _ = std::fs::remove_file(firmware_path);

    Ok(())
}
