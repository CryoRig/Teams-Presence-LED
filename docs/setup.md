# Development Environment Setup

This guide covers the one-time setup required to build and debug both the firmware and bridge components.

## Prerequisites

| Tool | Version | Purpose |
|------|---------|---------|
| [PlatformIO](https://platformio.org/) | Latest (VS Code extension) | Firmware build, flash, and debug |
| [Rust](https://www.rust-lang.org/) | 1.85+ (2024 edition) | Bridge application and diagnostics |

## Board

This project uses the **Seeed XIAO ESP32-S3**. It has a built-in USB-C connector that exposes both a CDC-ACM serial port (for debug logging) and a custom USB HID interface (VID `0x1209`, PID `0x0005`, Usage Page `0xFF00`) for communication with the bridge — no external FTDI or CH340 chip is needed.

- Product page: [Seeed XIAO ESP32-S3](https://wiki.seeedstudio.com/xiao_esp32s3_getting_started/)
- Upload protocol: `esptool` (via the built-in USB)
- Framework: Arduino (via [pioarduino](https://github.com/pioarduino/platform-espressif32.git))

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

> **Note:** On the XIAO ESP32-S3 via native USB, the board may not reset automatically after flashing even if the output says "Hard resetting via RTS pin...". If the new firmware does not start, unplug and re-plug the USB cable.

### Serial Monitor

```bash
cd firmware
pio device monitor
```

The monitor is configured with `send_on_enter` filter, local echo, and LF line endings to view debug logs at 115200 baud.

### Debugging

The PlatformIO debugger uses `esp-builtin` (on-chip JTAG via OpenOCD). To start a debug session:

1. Open the `firmware/` folder in VS Code
2. Set a breakpoint in `setup()` or `loop()`
3. Press F5 or use the PlatformIO Debug sidebar

## Diagnostic Tool

The bridge includes a standalone diagnostic tool to verify device enumeration via HID:

```bash
cd bridge/teams-presence-bridge-rs
cargo run --bin hid_diag
```

This scans for connected HID devices and verifies that a device matching Usage Page `0xFF00` is detected.

## Bridge Application

### Run in Development Mode

```bash
cd bridge/teams-presence-bridge-rs
cargo run
```

### Build Release Binary

```bash
cd bridge/teams-presence-bridge-rs
cargo build --release
```

The binary will be generated at `bridge/teams-presence-bridge-rs/target/release/TeamsPresenceBridge.exe`.

The bridge automatically detects and connects to the ESP32-S3 via USB HID (VID: `0x1209`, PID: `0x0005`). It reads settings and presence-to-command mappings from `config.json` (such as colors, animations, brightness, poll interval, and transition durations) and displays a Windows system tray icon with an `egui` settings window.

## Verification Checklist

Before full system use, confirm:

- [ ] The XIAO ESP32-S3 appears as a COM port in Device Manager (for flashing) and enumerates as a USB HID device
- [ ] `pio run --target upload` flashes successfully (re-plug USB if manual reset is needed)
- [ ] The LEDs run their rainbow boot animation upon startup
- [ ] `cargo run --bin hid_diag` discovers the HID device with Usage Page `0xFF00`
- [ ] `cargo run` launches the bridge, shows the tray icon, and synchronizes your Microsoft Teams presence state to the LEDs

