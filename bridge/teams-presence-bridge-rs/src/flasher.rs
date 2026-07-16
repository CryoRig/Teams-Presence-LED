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

    // 1. Scan serial ports for the ESP32-S3 in bootloader mode (VID: 0x303a)
    let ports = available_ports()?;
    let mut esp_port = None;

    for port in ports {
        if let SerialPortType::UsbPort(info) = &port.port_type {
            if info.vid == 0x303a {
                esp_port = Some(port.clone());
                break;
            }
        }
    }

    let port_info = match esp_port {
        Some(p) => p,
        None => {
            return Err("ESP32-S3 serial port (VID 0x303a) not found. Is it connected and in bootloader mode?".into());
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
        ResetBeforeOperation::DefaultReset,
        115_200,
    );

    let mut flasher = Flasher::connect(
        connection,
        true, // use_stub
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
