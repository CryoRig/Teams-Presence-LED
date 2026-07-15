# USB HID Protocol Specification

This document defines the binary protocol used for communication between the **Teams Presence Bridge** and the **LED Indicator** (ESP32). Communication has been migrated from serial CDC to a custom USB HID interface.

## Communication Parameters

- **Interface:** USB HID (Vendor Defined)
- **Usage Page:** `0xFF00`
- **Usage:** `0x01`
- **Output Report Size:** 64 bytes (1 byte Report ID + 63 bytes data)
- **Input Report Size:** 64 bytes (1 byte Report ID + 63 bytes data)

*Note: The ESP32 also exposes a secondary CDC Serial interface which can be used for debugging (`HELP` and `RESET` commands) at 115200 baud.*

## Command Set (Output Report)

The host sends commands to the device using a 5-byte payload structure:
`[Command ID] [Param 1] [Param 2] [Param 3] [Reserved]`

*Note: The ESP32 Arduino Core's `USBHIDVendor` uses Report ID `0x06` (`HID_REPORT_ID_VENDOR`). Depending on the host OS and library (`hidapi`), this Report ID byte (`0x06`) MUST be prepended to the buffer, making the actual transfer 6 bytes. The payload described below refers to the data bytes following the Report ID.*

### 0x03: Set Solid Color
Sets all LEDs in the chain to a single, static color.
- **Command ID:** `0x03`
- **Parameters:**
  - P1: Red component (0-255)
  - P2: Green component (0-255)
  - P3: Blue component (0-255)

### 0x04: Start Breathing Animation
Starts a continuous "breathing" (pulsing) effect using the specified color with a moderate speed (~3 second cycle).
- **Command ID:** `0x04`
- **Parameters:** Same as Solid Color (R, G, B)

### 0x05: Start Slow Breathing Animation
Starts a continuous "breathing" (pulsing) effect using the specified color with a slow speed (~5 second cycle). Intended for away/idle states.
- **Command ID:** `0x05`
- **Parameters:** Same as Solid Color (R, G, B)

### 0x06: Set Global Brightness
Sets the global brightness level for all LEDs.
- **Command ID:** `0x06`
- **Parameters:**
  - P1: Brightness level (0-255)
  - P2, P3: `0x00` (ignored)

### 0x02: Turn Off
Turns all LEDs off immediately.
- **Command ID:** `0x02`
- **Parameters:** `0x00` (ignored)

### 0x01: Heartbeat / Ping
Used by the Bridge to verify that the ESP32 is still connected and responsive.
- **Command ID:** `0x01`
- **Parameters:** `0x00` (ignored)
- **Response:** Sends an Input Report with Status Code `0x01` (PONG).

### 0x07: Set Transition Duration
Sets the duration for crossfade transitions between LED states.
- **Command ID:** `0x07`
- **Parameters:**
  - P1: High byte of duration in ms
  - P2: Low byte of duration in ms
  - P3: `0x00` (ignored)
  *(Duration = `(P1 << 8) | P2`, max 10000ms. 0 disables transitions.)*

### 0x08: Reset
Triggers a software reboot of the ESP32. Intended for development and diagnostic use.
- **Command ID:** `0x08`
- **Parameters:** `0x00` (ignored)

## Responses (Input Report)

The device may send an Input Report back to the host, formatted as:
`[Status Code] [Data] [Reserved...]`

- `0x01`: **PONG** - Response to a PING command.
- `0x02`: **OK** - Command received and processed successfully.
- `0xFF`: **ERROR** - Unknown command ID or invalid data.

## Error Handling & Edge Cases

1. **Loss of Communication:** If the ESP32 does not receive any command (or a `0x01 PING`) for **60 seconds**, it enters "Disconnected State" (a moderate white pulse) to alert the user.
2. **Invalid Commands:** Unrecognized command IDs will trigger an `0xFF ERROR` response.