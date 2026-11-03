# Standard Test Plan

This test plan should be executed after every feature addition or bug fix to ensure the Teams Presence LED system works from firmware up to the bridge application.

## Testing Scope
Depending on what components were modified, you only need to run the relevant tests:
- **Firmware Changes Only:** Complete **[Hardware Setup](#hardware-setup)** and **[Section 1: Firmware Tests](#1-firmware-tests)**.
- **Bridge Changes Only:** Complete **[Hardware Setup](#hardware-setup)** and **[Section 2: Bridge Tests](#2-bridge-tests)**. (If you need to test the LEDs reacting, also do **[Section 3: End-to-End Testing](#3-end-to-end-testing)**).
- **Changes to Both (or API/Protocol):** Complete **All Sections**.

---

## Hardware Setup
- Connect the **Seeed XIAO ESP32-S3** to your computer via USB.
- Verify the **WS2812B or SK6812 RGBW LEDs** are connected properly (the firmware defaults to an 8-LED WS2812B chain; the count (1-64) and LED type are configurable in the bridge settings and stored on the device):
  - Data pin to **GPIO 2** on the ESP32-S3.
  - Power (5V) and Ground (GND) are securely connected.

---

## 1. Firmware Tests
Use the command line to build and upload the firmware.

1. Open a terminal and navigate to the `firmware` directory:
   ```bash
   cd firmware
   ```
2. Put the device into bootloader mode (`cargo run --example hid_cmd -- bootloader` from the bridge directory, or hold BOOT while plugging in), then build and upload using PlatformIO:
   ```bash
   pio run -t upload --upload-port COMx
   ```
   See [docs/setup.md](docs/setup.md#build-and-flash) for the fallback `esptool` command.
3. **Enumeration Check:** Run `cargo run --bin hid_diag` from the bridge directory and verify the device is listed as `VID:1209 PID:0005 ... Teams Presence Bridge`.
   > **Note:** The upload output may say "Hard resetting via RTS pin...", but on the XIAO ESP32-S3 via native USB, it may not automatically reset. You may need to manually unplug and replug the USB cable to restart the device.
4. **Boot Animation Check:** Upon successful upload (and unplug/replug if necessary), verify the LEDs display a rainbow boot animation that scrolls across the strip and fades to black.
5. **Watchdog Check:** Wait for 60 seconds *without* starting the bridge application. The LEDs should begin a slow white breathing animation (Disconnected State).

---

## 2. Bridge Tests
Use the command line to run the Rust bridge application. Ensure the firmware is flashed and the device is connected before starting.

1. Open a terminal and navigate to the Rust bridge directory:
   ```bash
   cd bridge/teams-presence-bridge-rs
   ```
2. Run the application using Cargo:
   ```bash
   cargo run
   ```
3. **HID Connection Check:** Verify the bridge successfully detects the HID device (the XIAO ESP32-S3) in its console output/logs.
4. **Teams Log Parsing Check:** Verify the bridge detects and parses the local Teams log files (`%LOCALAPPDATA%\Packages\MSTeams_8wekyb3d8bbwe\LocalCache\Microsoft\MSTeams\Logs`) and reports presence states in its console output/logs. Quit Teams and verify the bridge reports `Offline`.
5. **System Tray UI Check:** 
   - Verify the system tray icon for the Teams Presence Bridge appears.
   - Right-click the tray icon and verify the context menu shows the ESP32 and Teams status lines, **Check for Updates**, and **Quit**.
   - Left-click the tray icon and verify the settings window opens (Autostart, intervals, brightness, presence mapping).
   - If changes were made to `config.json` logic, verify those settings apply correctly.
6. **LED Calibration Check:**
   - With calibration-capable firmware connected, open the settings window and verify the device calibration controls are enabled.
   - Adjust one RGB balance slider and gamma; verify the active LEDs change immediately without saving.
   - Select **White**, then **Show calibration pattern**, and verify steady white remains active beyond the old 2.8-second timeout. Adjust RGB balance and verify the white changes live. Change Teams presence and verify white remains active.
   - Select **Mid-gray** and verify the pattern switches immediately to dimmer white. With neutral gains, verify gamma changes affect brightness; at gamma 1.0, verify gains above 100% affect the selected channel until it clips. Verify slider changes do not end the pattern.
   - Select **Save to device** and verify the latest Teams state returns. Restart or reconnect the device and verify the calibration values remain active.
   - Adjust a balance slider, select **Show calibration pattern**, then **Cancel**: verify the pattern ends, the sliders snap back to the saved values, and the device output matches the saved profile.
   - While white calibration is active, stop the bridge and verify the normal disconnected breathing animation starts after 60 seconds.
   - Select **Reset to neutral**, verify the output returns to the uncorrected appearance, and reconnect to confirm the neutral profile persisted.
   - Connect firmware older than calibration support; verify the UI reports unsupported while presence, brightness, transitions, and heartbeat continue working.
7. **LED Count Check:**
   - With firmware 0.7.1 or newer connected, open the settings window and verify the **LED Hardware** section shows the **Number of LEDs** slider and "Saved on device" matching the lamp (8 on a fresh device).
   - Lower the slider below the physical chain length; verify the trailing LEDs turn off immediately and the remaining LEDs keep the presence color. Raise it again and verify they light up.
   - Select **Cancel**; verify the slider snaps back to the saved count and the LEDs match it.
   - Set the slider to the physical chain length and select **Save to device**; verify "Saved on device" updates. Reconnect or power-cycle the device and verify the boot animation and presence color use the saved count, and that `config.json` contains no LED count.
   - Run `cargo run --example hid_cmd -- set_led_count 0` and verify the reply is `ERROR`; run `get_led_count` and verify the count is unchanged.
   - Connect firmware older than 0.7.1; verify the section reports that newer firmware is required while presence control keeps working.
8. **LED Type Check:**
   - With firmware 0.7.2 or newer connected, verify the **LED Hardware** section shows the **LED type** dropdown with WS2812B and the three SK6812 variants, and "Saved on device" matching the lamp (WS2812B on a fresh device).
   - With an SK6812 RGBW strip attached, select its variant; verify the presence color is re-sent immediately in the correct colors (a wrong type shows shifted colors or dark trailing pixels). Set a white presence color or the **White** calibration pattern and verify only the white die lights; a saturated color such as green must not light the white die.
   - Select **Cancel**; verify the dropdown and the number of LEDs snap back to the saved values and the LEDs match them.
   - Select the correct type and **Save to device**; verify both "Saved on device" labels update. Power-cycle the device and verify the boot animation and presence color use the saved type and count, and that `config.json` contains neither.
   - Run `cargo run --example hid_cmd -- set_led_type 4` and verify the reply is `ERROR`; run `get_led_type` and verify the active type is unchanged and `count` is `4`.
   - Connect firmware 0.7.1; verify the LED count slider still works while the section reports that the LED type selector needs firmware 0.7.2.

---

## 3. End-to-End Testing (Visual Confirmation)
With both the firmware running and the bridge active, manually change your Teams presence states and confirm the LEDs accurately reflect each state.

| Teams State | LED Animation | LED Color |
| :--- | :--- | :--- |
| **Available** | Solid | Green |
| **AvailableIdle** | Breathe (Slow) | Orange |
| **Busy** / **Do Not Disturb** | Solid | Red |
| **BusyIdle** | Breathe (Slow) | Red |
| **Away** | Breathe (Slow) | Orange |
| **Be Right Back** | Breathe (Slow) | Orange/Red |
| **Offline** | Off | None |
| **Unknown** | Breathe | Gray |

> **Note:** If the bridge stops or loses connection, the watchdog timer will trigger after 60 seconds and revert the LEDs to the breathing white disconnected state.

---

## 4. Connection Resilience Checks

Ensure the system gracefully handles unexpected disconnects and varying start orders.

1. **Bridge Started After Device:** 
   - Plug in the ESP32.
   - Wait for the watchdog animation to begin (breathing white).
   - Start the bridge application.
   - Verify the LEDs immediately update to reflect your current Teams state.
2. **Device Plugged in After Bridge Started (and Teams Restart):**
   - Start the bridge application.
   - Plug in the ESP32.
   - Verify the LEDs update to reflect your current Teams state.
   - Close Microsoft Teams, then restart it.
   - Modify your presence state. Verify the LED color updates correctly.
3. **Mid-Session Disconnect & Reconnect:**
   - While both the bridge and Teams are running, plug in the ESP32 and ensure LEDs match your Teams state.
   - Close Microsoft Teams, restart it, and modify your state. Verify the LEDs update.
   - Unplug the ESP32. Wait until the bridge console/tray indicates the device was disconnected.
   - Re-plug the ESP32.
   - Repeat changing your Teams presence status. Verify the LEDs update correctly and consistently.
