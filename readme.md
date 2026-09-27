# Teams Presence USB LED Indicator

A dual-component system that reads your local Microsoft Teams presence and updates a connected USB LED strip. It runs entirely locally with no cloud or Graph API dependencies.

## Architecture

This project consists of two parts:

1. **Bridge Application (`bridge/teams-presence-bridge-rs/`)**
   A Rust application that runs in the Windows system tray. It parses local Teams log files and sends HID reports to the device via USB. It features a system tray icon for status monitoring and a settings window for configuring colors.

2. **Firmware (`firmware/`)**
   PlatformIO / Arduino firmware for the **Seeed XIAO ESP32-S3**. It exposes a custom USB HID interface, listens for binary commands, drives WS2812B LEDs using FastLED, and handles animations (solid, breathing).

## Features

- **No Cloud Required**: Parses local Teams log files for presence information.
- **Auto-Reconnect**: The bridge uses USB HID for reliable, driverless communication and reconnects if unplugged.
- **Customizable Colors**: Configure LED colors and animations for each presence state via the tray app settings.
- **Hardware Watchdog**: If the bridge application crashes or the computer sleeps, the device enters a disconnected state with a moderate white pulse after 60 seconds of no heartbeat.

## Documentation

- [Protocol Specification](docs/protocol.md) - Details the USB HID binary protocol used for communication between the bridge and firmware.

## License

MIT — see [LICENSE](LICENSE). © 2026 Sim-Lab.
