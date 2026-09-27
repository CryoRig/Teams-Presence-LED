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
- Verify the **WS2812B LEDs** (8-LED chain) are connected properly:
  - Data pin to **GPIO 2** on the ESP32-S3.
  - Power (5V) and Ground (GND) are securely connected.

---

## 1. Firmware Tests
Use the command line to build and upload the firmware.

1. Open a terminal and navigate to the `firmware` directory:
   ```bash
   cd firmware
   ```
2. Build and upload using PlatformIO:
   ```bash
   pio run -t upload
   ```
   > **Note:** The upload output may say "Hard resetting via RTS pin...", but on the XIAO ESP32-S3 via native USB, it may not automatically reset. You may need to manually unplug and replug the USB cable to restart the device.
3. **Boot Animation Check:** Upon successful upload (and unplug/replug if necessary), verify the LEDs display a rainbow boot animation that scrolls across the strip and fades to black.
4. **Watchdog Check:** Wait for 60 seconds *without* starting the bridge application. The LEDs should begin a slow white breathing animation (Disconnected State).

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
4. **Teams API Connection Check:** Verify the bridge successfully connects to the local Teams API and retrieves presence states.
5. **System Tray UI Check:** 
   - Verify the system tray icon for the Teams Presence Bridge appears.
   - Right-click the tray icon and verify the context menu options are responsive (e.g., Autostart, Updates, Settings).
   - If changes were made to `config.json` logic, verify those settings apply correctly.

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
