# Teams Presence USB LED Indicator

A dual-component system that reads your local Microsoft Teams presence and updates a connected USB LED strip. It runs entirely locally with no cloud or Graph API dependencies.

## Architecture

This project consists of two parts:

1. **Bridge Application (`bridge/`)**
   A .NET 8 WPF application that runs in the Windows system tray. It polls the local Teams API (with fallback to log parsing) and sends serial commands to the ESP32. It features a system tray icon for status monitoring and a settings window for configuring colors.

2. **Firmware (`firmware/`)**
   PlatformIO / Arduino firmware for the **Seeed XIAO ESP32-S3**. It listens for serial commands, drives WS2812B LEDs using FastLED, and handles animations (solid, breathing).

## Features

- **No Cloud Required**: Connects to the local Teams client API or parses local log files.
- **Auto-Reconnect**: The bridge automatically detects the ESP32 and reconnects if unplugged.
- **Customizable Colors**: Configure LED colors and animations for each presence state via the tray app settings.
- **Hardware Watchdog**: If the bridge application crashes or the computer sleeps, the ESP32 enters a disconnected state with a moderate white pulse after 30 seconds of no heartbeat.

## Documentation

- [Protocol Specification](docs/protocol.md) - Details the serial commands used for communication between the bridge and firmware.
- [Development Setup](docs/setup.md) - Instructions for building the firmware and running the bridge application.
