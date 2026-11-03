# USB HID Protocol Specification

This document defines the binary protocol used for communication between the **Teams Presence Bridge** and the **LED Indicator** (ESP32). Communication has been migrated from serial CDC to a custom USB HID interface.

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
- **Response:** Standard `0x02` OK before HID disconnects and the ROM serial port appears.

### 0x0A: Version
Queries the firmware version and hardware variant.
- **Command ID:** `0x0A`
- **Parameters:** `0x00` (ignored)
- **Response:** Sends an Input Report with Status Code `0x0A` followed by `[Major] [Minor] [Patch] [Variant]`. Variant `1` = Seeed XIAO ESP32-S3 (the only supported hardware); the bridge refuses to offer firmware updates for any other value.

### 0x0B: Get LED Calibration
Returns the active device calibration profile.
- **Parameters:** `0x00` (ignored)
- **Response:** Status `0x0B` followed by `[Red Gain] [Green Gain] [Blue Gain] [Gamma Tenths]`.
- Gains are attenuation percentages from `0` to `100`; `100` is unchanged. Gamma is stored in tenths from `5` to `30`; `10` is neutral (`1.0`).
- Firmware before 0.7.0 accepted gains up to `200`, which only clipped the channel. Such stored profiles are migrated once at boot by scaling all gains so the strongest channel becomes `100` (ratios preserved) and re-saved; values above `100` sent via `0x0C` are clamped to `100`.

### 0x0C: Preview LED Calibration
Applies a profile immediately in RAM. This does not write flash/NVS.
- **Parameters:** P1-P3 are red, green, and blue gains; P4 is gamma in tenths. Values outside supported ranges are clamped.
- **Response:** Standard `0x02` OK.

### 0x0D: Save LED Calibration
Persists the currently previewed profile in the device's NVS. Settings stay with the physical device. If a calibration test is active, ends it and restores the latest commanded presence state.
- **Parameters:** `0x00` (ignored)
- **Response:** Standard `0x02` OK.

### 0x0E: Calibration Test Pattern
Shows steady white or mid-gray using the active calibration profile until Save LED Calibration (`0x0D`) or the exit selector below is received. Preview changes apply live. Presence commands update the state to restore without interrupting the pattern. The normal communication watchdog still exits the test if the host stops sending commands.
- **Parameters:** P1 selects the pattern: `0` = white `(255,255,255)`, `1` = mid-gray `(128,128,128)`, `0xFF` = exit. Other values fall back to white. P2-P4 are ignored.
- **Exit (`P1 = 0xFF`):** Leaves the test pattern (if active), discards any previewed profile by reloading the saved profile from NVS, and restores the latest commanded presence state. Firmware older than 0.7.0 treats `0xFF` as white.
- **Response:** Standard `0x02` OK.

### 0x0F: Get LED Count
Returns the length of the LED chain the device currently drives. The count lives in the device's NVS, not in the bridge configuration, so it stays with the physical lamp.
- **Parameters:** `0x00` (ignored)
- **Response:** Status `0x0F` followed by `[Active Count] [Saved Count] [Maximum] [0x00]`. *Active* is the count in use right now (possibly previewed), *Saved* is the persisted value restored at boot, *Maximum* is the firmware buffer limit (`64`). Factory default is `8`.

### 0x10: Set LED Count
Resizes the active chain immediately. LEDs beyond the new count are blanked before the chain shrinks.
- **Parameters:**
  - P1: LED count `1`-`Maximum`; values above the maximum are clamped. `0` returns `0xFF` ERROR and changes nothing.
  - P2: `0` = preview only (RAM), `1` = apply and persist to NVS.
  - P3, P4: `0x00` (ignored)
- **Response:** Standard `0x02` OK.
- To cancel a preview the host sends `0x10` again with the saved count and `P2 = 0`.

### 0x11: Get LED Type
Returns the addressable LED chipset the device drives. Like the LED count, the type is stored in the device's NVS.
- **Parameters:** `0x00` (ignored)
- **Response:** Status `0x11` followed by `[Active Type] [Saved Type] [Type Count] [Flags]`. *Active* is the type in use right now (possibly previewed), *Saved* is the persisted value restored at boot, *Type Count* is the number of type IDs this firmware knows (valid IDs are `0` to `Type Count - 1`), so a bridge can offer exactly the types the firmware supports. *Flags* bit 0 = the active type needs a device restart to take effect (reserved for future chipsets whose signal timing differs from the booted driver; always `0` for the types below). Factory default is `0`.

| ID | Chipset | Channels | White die |
| --- | --- | --- | --- |
| `0` | WS2812B | RGB | - |
| `1` | SK6812 RGBW | RGBW | cool white (~6000 K) |
| `2` | SK6812 RGBNW | RGBW | neutral white (~4500 K) |
| `3` | SK6812 RGBWW | RGBW | warm white (~3000 K) |

IDs are append-only. For RGBW types the firmware moves the common component of each RGB value onto the white die (`W = min(R,G,B)`, RGB reduced by `W`), so protocol colors stay unchanged and white presence states light only the white die. The white color temperature is informational in firmware 0.7.2; the three SK6812 variants produce identical output.

### 0x12: Set LED Type
Switches the active chipset immediately; the current presence color is re-sent in the new pixel format.
- **Parameters:**
  - P1: type ID `0`-`Type Count - 1`. Unknown IDs return `0xFF` ERROR and change nothing.
  - P2: `0` = preview only (RAM), `1` = apply and persist to NVS.
  - P3, P4: `0x00` (ignored)
- **Response:** Standard `0x02` OK.
- To cancel a preview the host sends `0x12` again with the saved type and `P2 = 0`.

## Responses (Input Report)

The device may send an Input Report back to the host, formatted as:
`[Status Code] [Data] [Reserved...]`

- `0x00`: **KEEPALIVE** - Unsolicited once per second to keep the HID interface active; hosts ignore it. This does not count as a PONG or reset the device's command watchdog. Since firmware 0.7.0 the data byte carries the number of commands dropped from the device's input queue since the previous keepalive (`0` on a healthy link).
- `0x01`: **PONG** - Response to a PING command.
- `0x02`: **OK** - Command received and processed successfully.
- `0x0A`: **VERSION** - Followed by major, minor, patch, and hardware variant.
- `0x0B`: **CALIBRATION** - Followed by RGB gains and gamma tenths as described above.
- `0x0F`: **LED_COUNT** - Followed by active count, saved count, and maximum as described above.
- `0x11`: **LED_TYPE** - Followed by active type, saved type, type count, and flags as described above.
- `0xFF`: **ERROR** - Unknown command ID or invalid data.

Older firmware returns `0xFF` for calibration commands; the bridge treats that as calibration unsupported and continues normal presence control. Firmware before 0.7.1 likewise returns `0xFF` for the LED count commands, and the bridge hides the LED count controls. Firmware before 0.7.2 returns `0xFF` for the LED type commands, and the bridge hides the LED type selector.

## Error Handling & Edge Cases

1. **Loss of Communication:** If the ESP32 does not receive any command for **60 seconds**, it enters "Disconnected State" (a moderate white pulse, crossfaded in using the configured transition) to alert the user. Every command except `0x08 RESET` and `0x09 BOOTLOADER` refreshes this watchdog; the first command after a timeout restores the last commanded presence state.
2. **Invalid Commands:** Unrecognized command IDs will trigger an `0xFF ERROR` response.