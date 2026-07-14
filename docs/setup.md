# Development Environment Setup

This guide covers the one-time setup required to build and debug both the firmware and bridge components.

## Prerequisites

| Tool | Version | Purpose |
|------|---------|---------|
| [PlatformIO](https://platformio.org/) | Latest (VS Code extension) | Firmware build, flash, and debug |
| [Python 3](https://www.python.org/) | 3.8+ | Serial test tooling |
| [Rust](https://rustup.rs/) | 1.80+ (or stable) | Bridge application |

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

## Python Tools

Install dependencies for the serial test script:

```bash
cd tools
pip install -r requirements.txt
```

Run the interactive serial test tool:

```bash
python serial_test.py COM3
```

Replace `COM3` with the actual port shown in Device Manager.

## Bridge Application

The bridge is a Rust application that runs in the Windows system tray. It features a system tray icon for status monitoring and a settings window for configuring colors.

```bash
cd bridge/teams-presence-bridge-rs
cargo build --release
cargo run --release
```

The bridge reads `config.json` for the presence-to-command mapping and COM port settings. By default, it uses `"comPort": "AUTO"` to auto-detect the ESP32.

## Verification Checklist

Before integrating firmware and bridge, confirm:

- [ ] The XIAO ESP32-S3 appears as a COM port in Device Manager
- [ ] `pio run --target upload` flashes successfully and `Serial.println("BOOT")` appears in `pio device monitor`
- [ ] `tools/serial_test.py` can send `PING` and receive `PONG`
- [ ] The PlatformIO debugger attaches with `debug_tool = esp-builtin` and breakpoints work
