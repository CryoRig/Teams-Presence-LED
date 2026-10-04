# USB HID Protocol Specification

This document defines the binary protocol used for communication between the **Teams Presence Bridge** and the **LED Indicator** (ESP32). Communication has been migrated from serial CDC to a custom USB HID interface.

Alternative board firmware uses an opt-in framed serial transport instead of
USB HID. Its five-byte command/response payloads reuse the command IDs below;
see [Boards on Hand](available-boards.md) for framing, connection, and feature
differences. The original ESP32-S3 HID protocol remains unchanged.

## Communication Parameters

- **Interface:** USB HID (Vendor Defined)
- **Usage Page:** `0xFF00`
- **Usage:** `0x01`
- **Output Report Size:** 6 bytes (1 byte Report ID + 5 bytes data)
- **Input Report Size:** 6 bytes (1 byte Report ID + 5 bytes data)

*Note: In normal operation the device is HID-only (no CDC serial port). A USB-Serial/JTAG port only appears while the device is in ROM bootloader mode (see `0x09`).*

## Command Set (Output Report)

The host sends commands to the device using a 5-byte payload structure:
`[Command ID] [Param 1] [Param 2] [Param 3] [Param 4 / Reserved]`

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

### 0x09: Bootloader
Forces the device into ROM bootloader/download mode (ESP32-S3) so the bridge can flash new firmware over USB-Serial/JTAG.
- **Command ID:** `0x09`
- **Parameters:** `0x00` (ignored)

### 0x0A: Version
Queries the firmware version and hardware variant.
- **Command ID:** `0x0A`
- **Parameters:** `0x00` (ignored)
- **Response:** Sends an Input Report with Status Code `0x0A` followed by `[Major] [Minor] [Patch] [Variant]`. Variant `1` = Seeed XIAO ESP32-S3 (the only supported hardware); the bridge refuses to offer firmware updates for any other value.

### 0x0B: Get LED Calibration
Returns the active device calibration profile.
- **Parameters:** `0x00` (ignored)
- **Response:** Status `0x0B` followed by `[Red Gain] [Green Gain] [Blue Gain] [Gamma Tenths]`.
- Gains are percentages from `0` to `200`; `100` is unchanged. Gamma is stored in tenths from `5` to `30`; `10` is neutral (`1.0`).

### 0x0C: Preview LED Calibration
Applies a profile immediately in RAM. This does not write flash/NVS.
- **Parameters:** P1-P3 are red, green, and blue gains; P4 is gamma in tenths. Values outside supported ranges are clamped.
- **Response:** Standard `0x02` OK.

### 0x0D: Save LED Calibration
Persists the currently previewed profile in the device's NVS. Settings stay with the physical device.
- **Parameters:** `0x00` (ignored)
- **Response:** Standard `0x02` OK.

### 0x0E: Calibration Test Pattern
Shows red, green, blue, then white for 700 ms each using the active calibration profile, then restores the last commanded state.
- **Parameters:** `0x00` (ignored)
- **Response:** Standard `0x02` OK.

## Responses (Input Report)

The device may send an Input Report back to the host, formatted as:
`[Status Code] [Data] [Reserved...]`

- `0x01`: **PONG** - Response to a PING command.
- `0x02`: **OK** - Command received and processed successfully.
- `0x0B`: **CALIBRATION** - Followed by RGB gains and gamma tenths as described above.
- `0xFF`: **ERROR** - Unknown command ID or invalid data.

Older firmware returns `0xFF` for calibration commands; the bridge treats that as calibration unsupported and continues normal presence control.

## Error Handling & Edge Cases

1. **Loss of Communication:** If the ESP32 does not receive any command (or a `0x01 PING`) for **60 seconds**, it enters "Disconnected State" (a moderate white pulse) to alert the user.
2. **Invalid Commands:** Unrecognized command IDs will trigger an `0xFF ERROR` response.