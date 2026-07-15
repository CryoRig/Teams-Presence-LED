/// Quick diagnostic tool to list all HID devices visible to hidapi.
/// Run with: cargo run --release --bin hid_diag

fn main() {
    println!("=== HID Device Enumeration ===\n");

    let api = match hidapi::HidApi::new() {
        Ok(api) => api,
        Err(e) => {
            eprintln!("ERROR: Failed to initialize HID API: {}", e);
            std::process::exit(1);
        }
    };

    let mut count = 0;
    let mut vendor_matches = 0;

    for info in api.device_list() {
        count += 1;
        let is_match = info.usage_page() == 0xFF00;
        if is_match {
            vendor_matches += 1;
        }

        let marker = if is_match { " <<< MATCH (Usage Page 0xFF00)" } else { "" };

        println!(
            "[{}] VID:{:04X} PID:{:04X}  UsagePage:0x{:04X}  Usage:0x{:04X}  Interface:{}  {}{}",
            count,
            info.vendor_id(),
            info.product_id(),
            info.usage_page(),
            info.usage(),
            info.interface_number(),
            info.product_string().unwrap_or("(no name)"),
            marker,
        );
    }

    println!("\n--- Summary ---");
    println!("Total HID devices: {}", count);
    println!("Vendor page (0xFF00) matches: {}", vendor_matches);

    if vendor_matches == 0 {
        println!("\nWARNING: No device found with Usage Page 0xFF00.");
        println!("The ESP32 USBHIDVendor device is not being detected.");
        println!("\nPossible causes:");
        println!("  1. The ESP32 is not plugged in");
        println!("  2. The firmware wasn't flashed after the HID migration");
        println!("  3. Windows hasn't re-enumerated the device (try unplugging and replugging)");
        println!("  4. The ESP32 USB mode is wrong (needs ARDUINO_USB_MODE=0 for TinyUSB)");
    } else {
        println!("\nDevice found! The bridge should be able to connect.");
    }
}
