# Development Environment Setup

This guide covers the one-time setup required to build and debug both the firmware and bridge components.

## Prerequisites

| Tool | Version | Purpose |
|------|---------|---------|
| [PlatformIO](https://platformio.org/) | Latest (VS Code extension) | Firmware build, flash, and debug |
| [Rust](https://rustup.rs/) | 1.80+ (or stable) | Bridge application & diagnostic tools |

## Board

This project uses the **Seeed XIAO ESP32-S3**. It has a built-in USB-C connector that exposes both a CDC-ACM serial port and a JTAG debug interface over a single cable — no external FTDI or CH340 chip is needed.

- Product page: [Seeed XIAO ESP32-S3](https://wiki.seeedstudio.com/xiao_esp32s3_getting_started/)
- Upload protocol: `esptool` (via the built-in USB)

## Windows USB Driver (One-Time)

Before PlatformIO can flash or debug the ESP32-S3, the Espressif USB JTAG driver must be installed. Without it, PlatformIO reports `LIBUSB_ERROR_NOT_FOUND`.

Install via **one** of these methods:

1. **Espressif Installation Manager** (recommended):
   - Download from [Espressif's GitHub releases](https://github.com/espressif/idf-installer/releases)
   - Run the installer and select the JTAG driver option

2. **Command line**:
   ```powershell
   idf-env.exe --driver install --espressif
   ```

After installation, reconnect the XIAO board and verify it appears in Device Manager under **Ports (COM & LPT)**.

## Firmware

### Build and Flash

```bash
cd firmware
pio run --target upload
```

### Serial Monitor

```bash
cd firmware
pio device monitor
```

The monitor is configured with `send_on_enter` filter, local echo, and LF line endings to match the serial protocol.

### Debugging

The PlatformIO debugger uses `esp-builtin` (on-chip JTAG via OpenOCD). To start a debug session:

1. Open the `firmware/` folder in VS Code
2. Set a breakpoint in `setup()` or `loop()`
3. Press F5 or use the PlatformIO Debug sidebar

## Diagnostic Tools

A diagnostic tool is provided to list all HID devices visible to the system and confirm the ESP32 is correctly enumerated.

Run the diagnostic tool from the bridge directory:

```bash
cd bridge/teams-presence-bridge-rs
cargo run --release --bin hid_diag
```

## Bridge Setup

1. Make sure you have Rust and Cargo installed.
2. In a terminal, navigate to the `bridge/teams-presence-bridge-rs` directory.
3. Run the application:
   ```bash
   cargo run --release
   ```
4. The application will start in the system tray. It will automatically detect the ESP32 via USB HID and connect.

## Testing

You can use the system tray menu to view connection status. You can no longer send raw commands like `SOLID:255,0,0` through the serial monitor, as the device now expects binary HID reports. The serial monitor (115200 baud) is only used for `HELP`, `RESET`, and debug logging.

## Verification Checklist

Before integrating firmware and bridge, confirm:

- [ ] The XIAO ESP32-S3 appears as a COM port (for debug logging) AND as a USB Input Device (HID) in Device Manager
- [ ] `pio run --target upload` flashes successfully and `Serial.println("BOOT")` appears in `pio device monitor`
- [ ] The Rust diagnostic tool `hid_diag` lists the device under usage page `0xFF00`
- [ ] The PlatformIO debugger attaches with `debug_tool = esp-builtin` and breakpoints work
