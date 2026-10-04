# ISSUES.md
<!-- MACHINE-ORIENTED ISSUE REGISTRY. Schema per issue:
ID | SEV(CRITICAL>HIGH>MEDIUM>LOW>INFO) | COMPONENT | WHERE(file:line) | WHAT | WHY | FIX
Re-audit 2026-09-27 (pass 2) + fix pass: independent full review of bridge and firmware, then all
open findings were fixed in the same session. Every entry below is [FIXED] with a short note on what
was changed; verified by `cargo build --release`, `cargo clippy --all-targets -- -D warnings` (0 warnings),
`cargo test` (4 passed) and `pio run` (SUCCESS). On-device verification (2026-09-27, XIAO ESP32-S3):
flashed new firmware; device enumerates as 1209:0005 (H9); bridge connects, VERSION query answered
(variant 1), presence parsed from the Teams log and Offline mapping sent, three PING cycles OK,
HID write error on device drop → clean reconnect + re-query + re-send, process stayed alive (H6/L17 path).
Re-verified 2026-09-27 (pass 3): every [FIXED] claim below grep-checked against current source — all hold.
Remaining OPEN items are listed first; they require the user (license choice, visual LED checks) or a
published release (OTA path) and cannot be closed autonomously.
Removed as not applicable before the fix pass: M10, L12, L13, I6, I7, I11 (dup L11), I13 (dup H7).
Version bumped to 0.6.0 (bridge + firmware, enforced in lockstep by release.yml). -->

## OPEN

### O1 [FIXED] LICENSE file missing
- FIX: `LICENSE` (MIT, © 2026 Sim-Lab) added at repo root; Cargo.toml gained `authors`, `license = "MIT"`, `repository`, `description`; readme.md has a License section.

### O2 [VERIFIED] Hardware verification — L21 watchdog recovery on non-PING command
- RESULT 2026-09-27: `hid_cmd -- solid 0 200 0` → green; > 60 s → white watchdog pulse; `hid_cmd -- brightness 191` → reply OK, white pulse stopped and green returned. L21 confirmed on device.
- TOOL: `bridge/teams-presence-bridge-rs/examples/hid_cmd.rs` (sends any protocol command, prints the reply). Bridge must NOT be running (it would keep pinging).
- NOTE: device currently reports fw v0.5.0 (built after the L21 fix but before the version bump). Flash the 0.6.0 build before O4.

### O3 [VERIFIED] Hardware verification — H8 tray retry at logon
- RESULT 2026-09-27: Autostart enabled, sign-out/in performed; tray icon present, app survived. H8 confirmed by user.

### O4 [OPEN] Release-path verification pending — M11/M15/M16 bridge-driven OTA
- STEPS: release `v0.6.0` is published (assets `TeamsPresenceBridge.exe`, `seeed_xiao_esp32s3.bin`, `SHA256SUMS`, `manifest.json`; the next release will carry `SHA256SUMS.txt` covering all assets instead). Flash the *old* 0.5.0 firmware → tray → Check for Updates → Update Firmware → expect stages Downloading → WaitingForDevice → Connecting {COMx} → Erasing → Flashing → Verifying → Resetting → WaitingForReconnect → done, then version shows 0.6.0. Also plug in a second ESP dev board to confirm the "refusing to guess" error (M15).

## HIGH

### H5 [FIXED] `windows_subsystem = "windows"` was commented out
- WHERE: bridge/teams-presence-bridge-rs/src/main.rs:1
- FIX: Attribute uncommented; release builds no longer show a console window.

### H6 [FIXED] `panic = "abort"` + `.lock().unwrap()` → silent process death
- WHERE: Cargo.toml `[profile.release]`, main.rs, ui.rs, update_ui.rs
- FIX: `panic = "abort"` removed. Bridge thread body wrapped in `catch_unwind`, logged and restarted after 1 s. All `.lock().unwrap()` sites replaced by `LockOrRecover::lock_or_recover()` (main.rs), which recovers a poisoned mutex instead of propagating the panic into other threads.

### H8 [FIXED] Tray icon `build().unwrap()` aborted the app when the shell tray was not ready (autostart)
- WHERE: ui.rs `TeamsBridgeApp::new`
- FIX: Tray creation retried up to 30× (1 s apart). On persistent failure the app continues without a tray icon and shows the settings window instead of hiding it (`tray_icon` is now `Option`).

### H9 [FIXED] NEW (found during on-device test): custom USB VID/PID were never applied — device enumerated as stock XIAO 0x2886/0x0056
- WHERE: firmware/platformio.ini (`ARDUINO_USB_CDC_ON_BOOT=1`, `-D USB_VID/-D USB_PID`), firmware/src/UsbManager.cpp `begin()`
- WHAT: With `ARDUINO_USB_CDC_ON_BOOT=1` and `ARDUINO_USB_MODE=0` the Arduino core calls `USB.begin()` in `main.cpp` **before** `setup()`, so `USB.VID()/PID()/productName()` in `UsbManager::begin` were no-ops (`_started` already true). Additionally the board's `pins_arduino.h` unconditionally `#define`s `USB_VID 0x2886` / `USB_PID 0x0056`, overriding the `-D USB_VID=0x1209` flag (`build_unflags -D USB_VID` cannot remove a header define). Net effect: every build so far enumerated as 2886:0056; only the `is_xiao` fallback in hid.rs (removed in I16) made the bridge work. No CDC port ever appeared either.
- WHY: docs/setup.md, protocol.md, hid.rs and the flasher all assume 1209:0005. Removing the stale fallback (I16) would have broken the bridge completely.
- FIX: `ARDUINO_USB_CDC_ON_BOOT=0` so `USB.begin()` runs from our code after the descriptor is configured; defines renamed to `TPB_USB_VID/TPB_USB_PID`; `board_build.*vid/pid` lines removed. Verified: `hid_diag` shows `VID:1209 PID:0005 ... Teams Presence Bridge`. Docs updated (no CDC console; flashing via HID `BOOTLOADER` + `cargo run --example hid_cmd -- bootloader`).

## MEDIUM

### M9 [FIXED] Duplicate `with_inner_size` / `with_min_inner_size`
- WHERE: main.rs `ViewportBuilder`
- FIX: Dead pair removed; current size 420×600 / min 360×300.

### M11 [FIXED] Release workflow did not publish SHA256SUMS or manifest.json
- WHERE: .github/workflows/release.yml
- FIX: `manifest.json` step reads the bridge version from Cargo.toml and `FW_VERSION_*` from main.cpp. A `checksums` job (after both builds) downloads every release asset and publishes one `SHA256SUMS.txt` covering exe + firmware + manifest (renamed from extensionless `SHA256SUMS` after the 0.6.0 release for Windows friendliness; updater accepts both). Also added a `verify-version` job (tag must equal both versions) and a separate `ci.yml` (clippy `-D warnings` + test + `pio run` on push/PR). Actions bumped to Node 24 majors (checkout@v7, setup-python@v7, cache@v6); Linux jobs pinned to `ubuntu-24.04`.

### M12 [FIXED] `showSolid` cache not refreshed by BRIGHTNESS
- WHERE: firmware/src/main.cpp case 0x06
- FIX: `FastLED.show()` replaced by `showSolid(lastHardwareColor, true)` so the cached colour is re-pushed at the new brightness through the same path.

### M13 [FIXED] Update window unreachable (menu item hidden)
- WHERE: ui.rs tray menu
- FIX: "Check for Updates" menu item re-enabled (user decision); the startup update check result is now visible.

### M14 [FIXED] Teams exit never detected; `sysinfo` unused
- WHERE: teams.rs
- FIX: `TeamsClient::is_teams_running()` checks for `ms-teams.exe` via `sysinfo` every 5 s. When absent, `get_presence()` returns `"Offline"` (→ `Offline` mapping, default OFF) and clears the stale cached presence. Resolves I14 as well.

### M15 [FIXED] Flasher picked the first VID 0x303A serial port
- WHERE: flasher.rs, update_ui.rs
- FIX: Matches VID 0x303A **and** PID 0x1001 only. `list_esp_ports()` snapshots ESP ports before the bootloader command; those are excluded so only the newly enumerated port is accepted. Multiple candidates → refuse with an explicit error. Chosen port name surfaced via `FlashStage::Connecting { port }`.

### M16 [FIXED] Post-flash success assumed after a fixed 3 s sleep
- WHERE: update_ui.rs `start_firmware_update`
- FIX: After flashing, the bridge loop is resumed first, then `AppStatus.esp_connected` is polled for up to 20 s (`FlashStage::WaitingForReconnect`). If the device does not come back, the UI shows "Flash completed, but the device did not reconnect. Please unplug and re-plug the USB cable." `firmware_update_available` is no longer cleared blindly; ui.rs recomputes it when the version is re-queried.

## LOW

### H7 [FIXED] (re-rated HIGH→LOW) Pico/UF2 remnants
- WHERE: docs/protocol.md 0x09; firmware `IUsbManager.h` / `Esp32UsbManager.*` / `#ifdef ARDUINO_ARCH_ESP32` guards; bridge `firmware_download_url_esp32`, variant literal `1`, "(Variant N)" label
- FIX: protocol.md describes ESP32-S3 ROM bootloader only and documents variant `1` as the sole supported value. Firmware collapsed to a single concrete `UsbManager` (no interface, no arch guards, static instance instead of `new`); `HW_VARIANT` moved from platformio.ini into main.cpp. Bridge: field renamed `firmware_download_url`, constant `HW_VARIANT_XIAO_ESP32S3`, variant suffix dropped from the UI. Repo is single-target (Seeed XIAO ESP32-S3) by design.

### L11 [FIXED] TESTING.md drift ("Teams API", tray menu contents)
- FIX: Step 4 rewritten to log-file parsing + Offline-on-quit check; step 5 describes the actual menu (status lines, Check for Updates, Quit) and left-click settings window. Firmware section documents bootloader entry + `--upload-port` and an enumeration check via `hid_diag`. setup.md MSRV updated to 1.88 (see I10).

### L14 [FIXED] `drain_reads` relied implicitly on hidapi prepending the report ID
- FIX: Match arm is now `Ok(n) if n > 1 && buf[0] == HID_REPORT_ID_VENDOR` with a comment documenting the Windows hidapi behaviour. "No device found" log message now states the full match criteria (usage page + VID/PID or product string).

### L15 [FIXED] Required config fields without `serde(default)`; no range validation on load
- WHERE: config.rs
- FIX: All fields have defaults (`watchdog`, intervals, `presence_map`). `Config::clamp_ranges()` applied in `load_config` using shared `MIN/MAX_POLL_INTERVAL_MS` / `MIN/MAX_PING_INTERVAL_MS` constants, which the UI `DragValue` ranges now reference too. Unit test added.

### L16 [FIXED] `save_config` not atomic
- FIX: Writes `config.json.tmp` then `fs::rename` over the target; temp removed on rename failure. Unit test added.

### L17 [FIXED] PONG result discarded
- WHERE: hid.rs `send_ping`
- FIX: Consecutive missed PONGs counted; after `MAX_MISSED_PONGS = 3` the device is dropped so the normal reconnect path runs.

### L18 [FIXED] Downloaded firmware temp file leaked on flash failure
- FIX: `update_ui.rs` removes the temp file unconditionally after `flash_firmware_esp32` returns and via `fail_flash()` on the bootloader-timeout path; removal taken out of flasher.rs.

### L19 [FIXED] Flash stage labels mismatched
- FIX: `FlashStage` now has `Downloading`, `WaitingForDevice`, `Connecting { port }`, `Erasing`, `Flashing`, `Verifying`, `Resetting`, `WaitingForReconnect`, `Done`, `Error`; labels in update_ui.rs match the moment each is emitted.

### L20 [FIXED] Quit not blocked during flash
- FIX: Tray `quit` handler ignores the click (with a log line) while `UpdateUiState.flash_in_progress` is true. Bridge loop also checks `shutdown_flag` first in each iteration.

### L21 [FIXED] BRIGHTNESS/TRANSITION did not exit `STATE_DISCONNECTED`; UI ping max == watchdog timeout
- FIX: Firmware `markHostAlive()` restores `lastCommandedState` on PING, BRIGHTNESS and TRANSITION. Bridge caps `ping_interval_ms` at 30 000 ms (`MAX_PING_INTERVAL_MS`) both in the UI and on config load.

### L22 [FIXED] `xQueueSendFromISR` from TinyUSB task context
- FIX: Replaced with `xQueueSend(cmdQueue, &cmd, 0)`.

### L23 [FIXED] "Configuration saved successfully" did not auto-hide
- FIX: `ctx.request_repaint_after(remaining)` while the label is shown.

## INFO

### I8 [FIXED] Tail-window skipped after in-place log shrink
- FIX: Shrink check and `last_position == 0` tail-window are now two sequential `if`s.

### I10 [FIXED] Missing `rust-version`
- FIX: `rust-version = "1.88"` (let-chains are used throughout; 1.85 was insufficient). setup.md updated accordingly.

### I12 [FIXED] `presence_map`/`watchdog` cloned every 100 ms tick
- FIX: Cloned only inside the poll tick when a presence change is being handled.

### I14 [FIXED] Unused dependency `sysinfo`
- FIX: Now used for Teams process detection (M14).

### I15 [FIXED] Leftover UF2 branch in `verify_firmware_magic`
- FIX: Function reduced to the ESP `0xE9` magic check; URL parameter dropped.

### I16 [FIXED] Stale stock-XIAO VID/PID fallback
- FIX: `is_xiao` removed from hid.rs; match is VID/PID 0x1209/0x0005 or product string. NOTE: this exposed H9 — the firmware actually enumerated as 2886:0056 until H9 was fixed.

### I17 [FIXED] `println!` vs `eprintln!`; per-tick `Path::exists()`
- FIX: teams.rs uses `eprintln!`; `has_valid_log()` is evaluated once per poll tick and cached in the bridge loop.

### I18 [FIXED] Firmware readability nits
- FIX: `_onOutput` compares against `HID_REPORT_ID_VENDOR`; `sendResponse` comment corrected to the 5-byte report.

### I19 [DONE] Attribution
- FIX: Settings window footer "Teams Presence Bridge v{version} · Sim-Lab" is a hyperlink to the GitHub repo (`updater::GITHUB_REPO_URL`). License: see O1.

### I20 [PLANNED] Colorimeter-assisted calibration
- FOLLOW-UP: Add an optional workflow that measures LED output with a colorimeter and derives RGB/gamma calibration values. Current calibration is manual and visual only.
