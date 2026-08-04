# ISSUES.md
<!-- MACHINE-ORIENTED ISSUE REGISTRY. Schema per issue:
ID | SEV(CRITICAL>HIGH>MEDIUM>LOW>INFO) | COMPONENT | WHERE(file:line) | WHAT | WHY | FIX
Line numbers reflect repo state at 2026-07-25. Verify before applying automated fixes. After fix was deployed mark as done.
Re-audit 2026-07-25: every issue re-verified against source; [VERIFIED-2026-07-25] tag = fix confirmed in code + builds pass (cargo check --all-targets OK, pio run SUCCESS for seeed_xiao_esp32s3 and rpipico2). New findings C3, L11, I6–I9 appended. -->

## CRITICAL

### C1 [DONE][VERIFIED-2026-07-25] firmware-compile-broken sendVersion signature mismatch
- WHERE: firmware/src/Esp32UsbManager.h:12, firmware/src/PicoUsbManager.h:12 vs firmware/src/IUsbManager.h:16, firmware/src/Esp32UsbManager.cpp:28, firmware/src/PicoUsbManager.cpp:55
- WHAT: Headers declare `sendVersion(uint8_t major, uint8_t minor, uint8_t patch)` (3 params) marked `override`; interface + .cpp definitions use 4 params (`..., uint8_t variant`).
- WHY: 3-param `override` matches no virtual → compile error. 4-param pure virtual never overridden in class decl → class abstract → `new Esp32UsbManager()` / `new PicoUsbManager()` fail. Out-of-class 4-param definitions in .cpp are undeclared members → compile error. Firmware cannot build for either target.
- FIX: Change both header declarations to `void sendVersion(uint8_t major, uint8_t minor, uint8_t patch, uint8_t variant) override;`
- STATUS: fixed in firmware/src/Esp32UsbManager.h and firmware/src/PicoUsbManager.h (override signatures aligned to IUsbManager); validated with successful builds for env seeed_xiao_esp32s3 and env rpipico2.
- VERIFIED: 2026-07-25 — both headers declare 4-param `sendVersion(..., uint8_t variant) override` matching IUsbManager.h:17; `pio run` SUCCESS for both envs; HW_VARIANT defined via platformio.ini build flags (1=ESP32, 2=RP2350).

### C2 [DONE][VERIFIED-2026-07-25] build-script copies nonexistent exe
- WHERE: bridge/teams-presence-bridge-rs/build.ps1:26
- WHAT: Copies `target\release\teams-presence-bridge-rs.exe`, but Cargo.toml `[[bin]] name = "TeamsPresenceBridge"` produces `TeamsPresenceBridge.exe`.
- WHY: Publish step always fails ("executable not found"); release artifact never produced by script.
- FIX: `$exePath = ".\target\release\TeamsPresenceBridge.exe"`
- STATUS: fixed in bridge/teams-presence-bridge-rs/build.ps1 (exe copy path updated).
- VERIFIED: 2026-07-25 — build.ps1:26 uses `.\target\release\TeamsPresenceBridge.exe`, matching Cargo.toml `[[bin]] name`. (Note: `-NoOptimizations` switch is cosmetic-only → see I6.)

### C3 [DONE][VERIFIED-2026-07-25] NEW (regression from M2/M3 fix): bridge did not compile — unclosed delimiter in teams.rs
- WHERE: bridge/teams-presence-bridge-rs/src/teams.rs:110-127 (get_presence parse block)
- WHAT: The M2/M3 rework left the `for line in text.lines()` body mis-indented with one missing closing brace for the `if let Some(file_path)` block → `cargo check` failed with "unclosed delimiter" (E0601-class parse error). Entire bridge (bin + tests) unbuildable.
- WHY: Any fix touching this file was shipped unverified; build.ps1/publish would fail for anyone building from source.
- FIX: Re-indent the for-loop body and restore the missing `}` so the brace tree matches the intended nesting.
- STATUS: fixed 2026-07-25 during re-audit (loop body re-indented, missing brace restored).
- VERIFIED: 2026-07-25 — `cargo check --all-targets` passes (only pre-existing dead-code warnings, see I9); parse logic behavior unchanged from M2/M3 intent.

## HIGH

### H1 [DONE][VERIFIED-2026-07-25] config.json silently wiped on any parse error
- WHERE: bridge/.../src/main.rs:37-45 (load_config error branch)
- WHAT: On load/parse failure (e.g. one typo, trailing comma, locked file), default config is generated AND written over the existing config.json.
- WHY: Destroys all user customization (colors, mappings) irreversibly for a recoverable error.
- FIX: Only write default if file does not exist; on parse error rename bad file to `config.json.bak` before writing default, and surface error in UI.
- STATUS: fixed in bridge/teams-presence-bridge-rs/src/main.rs (default written only when missing; parse-failed config is backed up to config.json.bak before writing default; parse error is surfaced via log output).
- VERIFIED: 2026-07-25 — main.rs:39-64: `!config_path.exists()` branch writes default; parse-error branch renames to config.json.bak (rename failure also logged) before writing default.

### H2 [DONE][VERIFIED-2026-07-25] process aborts if HID init fails (panic=abort)
- WHERE: bridge/.../src/hid.rs:29 `HidApi::new().expect(...)`; Cargo.toml `panic = "abort"`
- WHAT: `expect` inside background bridge thread; with `panic=abort` any panic (also every `.lock().unwrap()` on poisoned mutex) kills the whole process with no UI feedback.
- WHY: Transient hidapi init failure → app disappears silently at startup/tray.
- FIX: `HidManager::new() -> Result<Self,_>`; retry/report in bridge loop. Audit `.unwrap()` on mutexes (25+ sites) or accept as invariant and document.
- STATUS: fixed in bridge/teams-presence-bridge-rs/src/hid.rs and src/main.rs (HidManager::new now returns Result; bridge loop retries HID init every 5s and logs failures instead of aborting process).
- VERIFIED: 2026-07-25 — hid.rs:30 `new() -> Result<Self, hidapi::HidError>`; main.rs bridge loop retries via `last_hid_init_attempt` every 5s. Mutex `.lock().unwrap()` sites remain (accepted invariant per original FIX note; panic=abort still in Cargo.toml).

### H3 [DONE][VERIFIED-2026-07-25] LED stuck OFF after host sleep/resume (state-loss protocol gap)
- WHERE: firmware/src/main.cpp:120-127 (PING handler) + bridge/.../src/main.rs:236-249 (presence diffing)
- WHAT: Watchdog fires → STATE_DISCONNECTED. Next PING sets STATE_OFF. Bridge only re-sends presence when `presence != previous_presence`, so if USB never re-enumerated (no reconnect path), no color command is sent.
- WHY: After PC sleep >60s with USB staying connected, LED shows OFF while Teams presence is unchanged (e.g. still Busy) until the next real presence change. Wrong-signal failure for the core use case.
- DEPENDS-ON: C1 (if applying firmware fix option a), M1 (if applying bridge fix option b using PONG-gap detection).
- FIX (any of): (a) firmware: on PING while DISCONNECTED, restore pre-watchdog state instead of STATE_OFF; (b) bridge: reset `previous_presence = None` when a PONG follows a ping gap > WATCHDOG_TIMEOUT; (c) bridge: periodically re-send current presence (idempotent).
- STATUS: fixed in firmware/src/main.cpp using option (a): firmware now restores the last commanded presence state/color on PING after watchdog disconnect instead of forcing OFF.
- VERIFIED: 2026-07-25 — main.cpp PING handler restores `lastCommandedState`/`lastCommandedColor` (tracked in all state-setting commands) with transition when currentState==STATE_DISCONNECTED; bridge additionally resets `previous_presence = None` on reconnect (belt-and-suspenders).

### H4 [DONE][VERIFIED-2026-07-25] no integrity verification of downloaded firmware/updates
- WHERE: bridge/.../src/updater.rs:129-141 (download_firmware), flasher.rs:139 (write_bin_to_flash @0x10000)
- WHAT: Firmware binary downloaded from GitHub release and flashed with no checksum/signature validation, no size/type sanity check (e.g. ESP32 image magic 0xE9).
- WHY: Corrupt/truncated download or wrong asset bricks the app partition; supply-chain trust rests solely on TLS+GitHub account security.
- FIX: Publish SHA256SUMS as release asset; verify hash before flashing; sanity-check first byte 0xE9 for ESP32 app images and UF2 magic ("UF2\n" @0 and family ID) for uf2.
- STATUS: fixed in bridge/teams-presence-bridge-rs/src/updater.rs and src/update_ui.rs (+ Cargo.toml `sha2`): firmware download now requires SHA256SUMS asset verification and performs binary format sanity checks (ESP .bin 0xE9, UF2 magic header) before writing temp file/flash path.
- VERIFIED: 2026-07-25 — updater.rs `download_firmware` errors out if SHA256SUMS asset absent ("refusing unverified firmware download"); `verify_firmware_magic` checks 0xE9 / "UF2\n"; `verify_firmware_sha256` matches asset name against sums file and compares case-insensitively.

## MEDIUM

### M1 [DONE][VERIFIED-2026-07-25] device→host responses likely dropped / demux race
- WHERE: firmware/src/Esp32UsbManager.cpp:33-36 & PicoUsbManager.cpp:60-63 (sendResponse, 2-byte report) vs descriptor report size 5; bridge/.../src/hid.rs:76-90,102-113 (shared read stream for PONG + VERSION)
- WHAT: (a) `sendResponse` sends 2-byte input report but HID descriptor declares 5-byte reports; Windows HID class driver discards size-mismatched input reports → PONG likely never received (ping check silently no-ops). (b) PONG and VERSION replies share one unsynchronized read queue; a stale PONG can consume the read slot of `query_firmware_version` and vice versa.
- WHY: Ping health-check is ineffective (disconnect only detected on write error); version query intermittently returns None.
- DEPENDS-ON: C1 (firmware-side portion cannot be shipped until firmware compiles).
- FIX: Pad all input reports to full report size (5 bytes); in bridge, read-loop that drains reports and dispatches on first payload byte instead of one-shot `read_timeout` per command.
- STATUS: fixed in firmware (padded sendResponse to 5 bytes) and bridge (added robust drain_reads loop to handle PONG/VERSION without drops).
- VERIFIED: 2026-07-25 — both `sendResponse` impls send 5-byte reports matching descriptor Report Count (5); hid.rs `drain_reads` loops with 10ms read_timeout slices (100ms/500ms budget), dispatches on payload byte (PONG=0x01, VERSION=0x0A), and drains stale queued OK responses.

### M2 [DONE][VERIFIED-2026-07-25] teams.rs loses presence lines split across incremental reads
- WHERE: bridge/.../src/teams.rs:96-121
- WHAT: Reads from `last_position` to EOF; if Teams is mid-write, last line is partial; `last_position += contents.len()` consumes the fragment; the completed line's prefix is never seen on next read.
- WHY: Presence transitions can be silently missed (stale LED until next event).
- FIX: Only advance `last_position` to the byte offset after the last `'\n'`; reprocess remainder next poll.
- STATUS: fixed in bridge/teams-presence-bridge-rs/src/teams.rs (last_position only advances to the last newline byte offset).
- VERIFIED: 2026-07-25 — `rposition(b'\n')` gates parsing; only `nl_pos+1` bytes consumed, remainder reprocessed next poll. WARNING: the fix as previously committed did NOT compile (unclosed delimiter) — regression logged and repaired as C3.

### M3 [DONE][VERIFIED-2026-07-25] teams.rs stalls permanently on non-UTF8 byte
- WHERE: bridge/.../src/teams.rs:99-101 `read_to_string`
- WHAT: Any invalid UTF-8 in the log → `read_to_string` errors every poll, `last_position` never advances.
- WHY: Presence parsing dead-locks until log rotation; also re-reads the same region each poll (I/O churn).
- FIX: Read `Vec<u8>` + `String::from_utf8_lossy`.
- STATUS: fixed in bridge/teams-presence-bridge-rs/src/teams.rs (replaced read_to_string with read_to_end and from_utf8_lossy).
- VERIFIED: 2026-07-25 — reads `Vec<u8>` via read_to_end, decodes with `String::from_utf8_lossy`; no UTF-8 error path remains.

### M4 [DONE][VERIFIED-2026-07-25] UF2 flasher targets any UF2 drive, not the project's device
- WHERE: bridge/.../src/flasher.rs:160-185
- WHAT: Picks first removable drive containing INFO_UF2.TXT.
- WHY: Another UF2 device in bootloader (different Pico, Feather, micro:bit-style) gets flashed with rpipico2 firmware.
- FIX: Parse INFO_UF2.TXT `Board-ID`/`Model` and require RP2350 match before copying.
- STATUS: fixed in bridge/teams-presence-bridge-rs/src/flasher.rs (reads INFO_UF2.TXT to enforce Board-ID: RP2350 string match).
- VERIFIED: 2026-07-25 — INFO_UF2.TXT is read and must contain RP2350 before copy. Caveat: the `|| contains("RP2350")` clause makes the strict `Board-ID:` check redundant/looser → see I7.

### M5 [DONE][VERIFIED-2026-07-25] single release tag conflated for two versioned artifacts
- WHERE: bridge/.../src/updater.rs:107-127 (check_updates); Cargo.toml version=0.5.0 vs firmware FW 0.5.0 (main.cpp:18-20)
- WHAT: One `tag_name` semver compared against both bridge and firmware current versions, which already diverge.
- WHY: With tag v0.5.0: bridge reports update available even if no bridge changes; asymmetric versioning makes "up to date" ambiguous; flashing "latest" can downgrade or no-op.
- FIX: Release manifest asset (JSON: {bridge: x.y.z, firmware: x.y.z}) or separate tags; compare per-artifact.
- STATUS: fixed in bridge/teams-presence-bridge-rs/src/updater.rs (supports parsing manifest.json for separate version tracking).
- VERIFIED: 2026-07-25 — `fetch_latest_release` parses manifest.json asset into separate `version` (bridge) and `firmware_version`; `check_updates` compares per-artifact. Falls back to tag semver for both when manifest absent (documented behavior; ensure releases ship manifest.json).

### M6 [DONE][VERIFIED-2026-07-25] boot animation leaves dim rainbow residue; cache desync
- WHERE: firmware/src/main.cpp:95-115 (bootAnimation) + 63-70 (showSolid cache)
- WHAT: bootAnimation writes `leds[]`/brightness directly without updating `lastHardwareColor`. Final `showSolid(CRGB::Black)` early-returns (cache already Black) → no `FastLED.show()` after brightness restore. Fade loop `for(b=255;b>=0;b-=8)` ends at b=7, never 0.
- WHY: Strip retains last rainbow frame (visible at brightness 7 scaling artifacts) until first host command; STATE_OFF frames also early-return.
- DEPENDS-ON: C1 (firmware compile blocker).
- FIX: End bootAnimation with `showSolid(CRGB::Black, true)` (force) and iterate fade to exactly 0.
- STATUS: fixed in firmware/src/main.cpp (fades exactly to 0 before restoring brightness and forces a black showSolid).
- VERIFIED: 2026-07-25 — bootAnimation ends with explicit `setBrightness(0)+show()`, then restores BRIGHTNESS and calls `showSolid(CRGB::Black, true)` (forced), which also syncs `lastHardwareColor`.

### M7 [DONE][VERIFIED-2026-07-25] protocol.md / readme.md drift from implementation
- WHERE: docs/protocol.md (whole), readme.md:11,22
- WHAT: protocol.md omits 0x09 BOOTLOADER and 0x0A VERSION; claims 64-byte reports (actual: 5 data bytes + report ID 0x06); references CDC serial `HELP`/`RESET` commands not present in firmware source. readme claims "polls the local Teams API (with fallback to log parsing)" — only log parsing exists; claims watchdog "after 30 seconds" — actual WATCHDOG_TIMEOUT=60000ms.
- WHY: Any agent/dev implementing against docs produces wrong host code.
- FIX: Sync docs: add 0x09/0x0A, correct report sizes, remove Teams-API and serial-command claims, watchdog=60s.
- STATUS: fixed in docs/protocol.md and readme.md.
- VERIFIED: 2026-07-25 — protocol.md documents 0x09/0x0A, 6-byte reports (1 report ID + 5 data), PONG/OK/ERR responses, watchdog=60s; readme now says "parses local Teams log files" and "after 60 seconds". Residual drift found in docs/setup.md (HELP/RESET/BOOT claims) → tracked as L11.

### M8 [VERIFIED-OK — NOT AN ISSUE, do not re-flag]
- WHERE: bridge/.../src/updater.rs:74 `X-GitHub-Api-Version: 2026-03-10`
- WHAT: Verified 2026-07-25 against docs.github.com/en/rest/about-the-rest-api/api-versions: `2026-03-10` is a currently supported API version (the latest; `2022-11-28` is also supported until 2028-03-10). Header value is correct.
- NOTE: Unsupported versions return `410 Gone` (not 400). No action required.
- VERIFIED: 2026-07-25 — re-confirmed header unchanged in updater.rs; still not an issue.

## LOW

### L1 [DONE][VERIFIED-2026-07-25] predictable temp file name for downloaded firmware
- WHERE: bridge/.../src/updater.rs:130,143-149 (uuid_like_random = micros timestamp)
- WHAT: Temp path `teams_presence_fw_<timestamp>.bin` in shared %TEMP% is guessable; no O_EXCL-style creation.
- WHY: Local attacker could pre-create/swap file between download and flash (TOCTOU) on multi-user machines.
- FIX: `tempfile` crate (NamedTempFile) + flash from held handle, or verify hash at flash time (see H4).
- STATUS: fixed in bridge/teams-presence-bridge-rs/src/updater.rs and Cargo.toml (switched to tempfile-created unique file and persisted path).
- VERIFIED: 2026-07-25 — `tempfile::Builder` NamedTempFile with prefix/suffix, persisted via `keep()`; combined with H4 hash verification this satisfies the FIX options.

### L2 [DONE][VERIFIED-2026-07-25] has_valid_log lies after first success
- WHERE: bridge/.../src/teams.rs:41-43
- WHAT: Returns true if a log file was ever found; never re-validated (file deleted, Teams uninstalled).
- WHY: Tray shows "Teams: Log Parsing Active" indefinitely.
- FIX: Check `current_file` existence and/or recency of successful reads.
- STATUS: fixed in bridge/teams-presence-bridge-rs/src/teams.rs (has_valid_log now verifies current_file exists).
- VERIFIED: 2026-07-25 — `has_valid_log` = `current_file.map_or(false, |p| p.exists())`.

### L3 [DONE][VERIFIED-2026-07-25] stale config field comPort
- WHERE: bridge/teams-presence-bridge-rs/config.json:2
- WHAT: `"comPort": "AUTO"` not in Config struct; silently ignored (serde default tolerates unknown fields).
- WHY: Misleads users into thinking COM port is configurable; leftover from pre-HID serial design.
- FIX: Remove field; optionally `#[serde(deny_unknown_fields)]` (conflicts with forward-compat — decide policy).
- STATUS: fixed in bridge/teams-presence-bridge-rs/config.json (removed stale comPort field).
- VERIFIED: 2026-07-25 — config.json contains no comPort key; fields align with Config struct (camelCase).

### L4 [DONE][VERIFIED-2026-07-25] log spam on every reconnect poll
- WHERE: bridge/.../src/hid.rs:55 ("No device found" eprintln), main.rs reconnect every poll_interval (5s)
- WHAT: Unconditional stderr noise while device unplugged.
- FIX: Log once per state transition.
- STATUS: fixed in bridge/teams-presence-bridge-rs/src/hid.rs (added stateful suppression for repeated "No device found" logs until reconnect).
- VERIFIED: 2026-07-25 — `missing_device_logged` flag set after first miss, cleared on successful connect.

### L5 [DONE][VERIFIED-2026-07-25] console window not hidden in release
- WHERE: bridge/.../src/main.rs:1 `//#![windows_subsystem = "windows"]` commented out
- WHAT: Tray app shows a console window for end users.
- FIX: `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]`
- STATUS: fixed in bridge/teams-presence-bridge-rs/src/main.rs (release-only windows_subsystem attribute enabled).
- VERIFIED: 2026-07-25 — main.rs:1 `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]`.

### L6 [DONE][VERIFIED-2026-07-25] brightness/transition live-preview mutates shared config without persistence flag
- WHERE: bridge/.../src/ui.rs:224-244
- WHAT: Sliders write directly to shared config (device updates live) but revert on restart unless "Save Configuration" clicked; no dirty indicator.
- WHY: Perceived settings loss.
- FIX: Dirty-state marker on Save button or autosave on slider release.
- STATUS: fixed in bridge/teams-presence-bridge-rs/src/ui.rs (added dirty-state tracking, Save button marker, and unsaved-changes indicator).
- VERIFIED: 2026-07-25 — `config_dirty` set by all editable controls (intervals, sliders, presence map, watchdog); Save button renders "Save Configuration *" plus "Unsaved configuration changes" label; cleared on successful save.

### L7 [DONE][VERIFIED-2026-07-25] initial log read loads whole file into memory
- WHERE: bridge/.../src/teams.rs:96-101 (first pass, last_position=0)
- WHAT: Entire existing Teams log (can be tens of MB) read as one String at startup.
- FIX: On first attach, seek to max(0, len - N) tail window (e.g. 256KB).
- STATUS: fixed in bridge/teams-presence-bridge-rs/src/teams.rs (initial read now seeks to a 256KB tail window).
- VERIFIED: 2026-07-25 — `INITIAL_READ_TAIL_BYTES = 256*1024` applied when `last_position == 0`. Edge case: window is skipped right after an in-place file shrink (else-if chain) → see I8.

### L8 [DONE][VERIFIED-2026-07-25] unknown presence rendered as watchdog color
- WHERE: bridge/.../src/main.rs:239-244
- WHAT: Presence value missing from presence_map sends the watchdog (white breathe) command — same visual as bridge-dead state.
- WHY: Semantic collision; user cannot distinguish "new Teams status string" from "bridge crashed".
- FIX: Distinct fallback entry (`Unknown` already exists in map — route unmapped values to it).
- STATUS: fixed in bridge/teams-presence-bridge-rs/src/main.rs (unmapped presence now uses `Unknown` mapping; watchdog is only final fallback if `Unknown` is missing).
- VERIFIED: 2026-07-25 — fallback chain `presence_map.get(p)` → `presence_map.get("Unknown")` → watchdog, each with distinct log lines; `Unknown` present in both Config::default() and shipped config.json.

### L9 [DONE][VERIFIED-2026-07-25] Pico USB descriptor timing may miss custom VID/PID
- WHERE: firmware/src/PicoUsbManager.cpp:41-49
- WHAT: `TinyUSBDevice.setID/setManufacturer/setProduct` called in setup(); on earlephilhower+Adafruit TinyUSB the device may already be attached → descriptors sometimes not applied without detach()/attach() cycle.
- WHY: Device could enumerate with default Pico VID/PID → bridge (matching 0x1209:0x0005) never connects.
- DEPENDS-ON: C1 (firmware compile blocker).
- FIX: Standard pattern: `if (TinyUSBDevice.mounted()) { TinyUSBDevice.detach(); delay(10); TinyUSBDevice.attach(); }` after descriptor setup. VERIFY on hardware.
- STATUS: fixed in firmware/src/PicoUsbManager.cpp (added detach/attach cycle).
- VERIFIED: 2026-07-25 — code review: `if (TinyUSBDevice.mounted()) { detach(); delay(10); attach(); }` present after descriptor setup. Hardware enumeration re-test still advisable (code-level verification only).

### L10 [ACCEPTED][VERIFIED-2026-07-25] blocking boot animation delays USB readiness
- WHERE: firmware/src/main.cpp:96-115 (≈1.5s+ of delay() in setup)
- WHAT: Commands arriving during bootAnimation are queued/dropped before callback registration completes its useful effect; host may connect and ping into a blocked loop.
- DEPENDS-ON: C1 (firmware compile blocker).
- FIX: Make animation non-blocking in loop(), or acceptable as-is (document).
- STATUS: accepted as-is; the 1.5s boot delay is considered fine.
- VERIFIED: 2026-07-25 — behavior unchanged (blocking bootAnimation in setup()); acceptance stands.

### L11 [OPEN] NEW: docs/setup.md still references removed serial commands and stale claims (M7 residue)
- WHERE: docs/setup.md "Testing" section (≈line 96) and Verification Checklist (≈line 103)
- WHAT: States the serial monitor "is only used for `HELP`, `RESET`, and debug logging" — no HELP/RESET serial command handling exists in firmware source (HID 0x08 RESET only). Checklist also expects `Serial.println("BOOT")` in the monitor; no such print exists in firmware/src/main.cpp. setup.md is also ESP32-only and does not mention the rpipico2 target that platformio.ini/flasher support.
- WHY: Same doc-drift class as M7; devs following setup.md will look for output/commands that don't exist.
- FIX: Remove HELP/RESET sentence, drop or update the BOOT checklist item, optionally add a short rpipico2 build note.

## INFO

### I1 [DONE][VERIFIED-2026-07-25] power-limit comment mismatch
- WHERE: firmware/src/main.cpp:203 — `setMaxPowerInVoltsAndMilliamps(5, 480)` comment says "500mA".
- FIX: Align comment or value.
- STATUS: fixed in firmware/src/main.cpp (comment updated to 480mA to match configured limit).
- VERIFIED: 2026-07-25 — comment reads "5V 480mA".

### I2 [DONE][VERIFIED-2026-07-25] 8 WS2812B at full white exceed 480mA budget by design
- WHERE: firmware/src/main.cpp:24-26 — 8 LEDs × ~60mA = 480mA = exactly the cap; FastLED will dim whites. Expected behavior; document that white states render dimmer.
- STATUS: documented in firmware/src/main.cpp comments.
- VERIFIED: 2026-07-25 — setMaxPower comment notes "full white will be dimmed by FastLED to meet this budget".

### I3 [ACCEPTED][VERIFIED-2026-07-25] hid.rs comment/enum duplication of report ID
- WHERE: bridge/.../src/hid.rs:10, firmware/src/PicoUsbManager.cpp:7 — HID_REPORT_ID_VENDOR=6 duplicated across languages with no shared source of truth; drift risk. Consider generating protocol constants from docs/protocol.md or a shared spec file.
- STATUS: accepted as-is (overhead of shared spec generator not worth it for 5 constants).
- VERIFIED: 2026-07-25 — constants still consistent across hid.rs (0x06) and PicoUsbManager.cpp (6); no drift.

### I4 [DONE][VERIFIED-2026-07-25] dependency hygiene (versions verified online 2026-07-25)
- WHERE: bridge Cargo.toml + docs/setup.md:10
- WHAT: sysinfo pinned "0.30" (0.30.13 = 2024-07); current is 0.39.6 (2026-07) — ~9 minor releases behind, many breaking API changes across 0.31/0.33/0.35 (Disks API refactors relevant to flasher.rs). eframe "0.35" is CURRENT (0.35.0, 2026-06) — but its MSRV is Rust 1.92, edition 2024. setup.md states "Rust 1.80+" which cannot build this project.
- FIX: setup.md prerequisite → Rust 1.92+. Optionally bump sysinfo to 0.39 (verify Disks::new_with_refreshed_list / is_removable / mount_point API compatibility in flasher.rs).
- STATUS: fixed in setup.md and Cargo.toml.
- VERIFIED: 2026-07-25 — setup.md prerequisite says "Rust 1.92+"; Cargo.toml `sysinfo = "0.39"`; flasher.rs uses 0.39-compatible `Disks::new_with_refreshed_list`/`is_removable`/`mount_point`; cargo check passes.

### I5 [DONE][VERIFIED-2026-07-25] dead UI parameter
- WHERE: bridge/.../src/update_ui.rs:299-304 — variant `.unwrap_or(1)` unreachable in practice (firmware_url gate returns earlier when firmware_current is None); harmless but masks logic intent.
- STATUS: fixed in update_ui.rs.
- VERIFIED: 2026-07-25 — `.unwrap_or(1)` removed; variant now resolved via gated match `(latest_release, variant)` and plain `.unwrap()` that is provably reachable only when variant is Some (firmware_url gate returns earlier otherwise).

### I6 [OPEN] NEW: build.ps1 -NoOptimizations switch is cosmetic-only
- WHERE: bridge/teams-presence-bridge-rs/build.ps1:1-23
- WHAT: `-NoOptimizations` only changes the printed mode string; `$baseArgs` is always `build --release`, and [profile.release] (LTO, strip, opt-level=z) always applies.
- WHY: Misleading developer UX; the flag silently does nothing.
- FIX: Remove the switch, or wire it to a real profile (e.g. `cargo build --profile release-noopt` with a dedicated profile).

### I7 [OPEN] NEW: UF2 board match accepts any INFO_UF2.TXT containing "RP2350" (M4 fix looser than stated)
- WHERE: bridge/teams-presence-bridge-rs/src/flasher.rs:176 `contains("Board-ID: RP2350") || contains("RP2350")`
- WHAT: The second disjunct makes the first redundant and matches "RP2350" anywhere in the file (e.g. a non-RP2350 board whose INFO text mentions RP2350 compatibility).
- WHY: Weakens the M4 guarantee of flashing only the project's device class; residual (low) risk of flashing a wrong RP2350-family board remains regardless.
- FIX: Parse the `Board-ID:` line explicitly and require it to start with `RP2350`; drop the bare `contains("RP2350")` fallback.

### I8 [OPEN] NEW: tail-window skipped after in-place log shrink (L7 edge case)
- WHERE: bridge/teams-presence-bridge-rs/src/teams.rs:91-96 (shrink check else-if chain)
- WHAT: When `metadata.len() < last_position`, last_position is reset to 0 but the `else if last_position == 0` tail-window branch is not re-evaluated in the same pass → the shrunken file is read in full from offset 0 once.
- WHY: One-off full-file read after in-place truncation; bounded impact (next pass behaves normally). Rotation to a new file path is unaffected (position reset happens before open).
- FIX: After resetting to 0 on shrink, also apply `saturating_sub(INITIAL_READ_TAIL_BYTES)` (or restructure as two sequential ifs).

### I9 [OPEN] NEW: dead code — unused field/variant warnings and unreachable UI branch
- WHERE: bridge/.../src/updater.rs:40 (`UpdateCheckResult.latest` never read), src/flasher.rs:18 (`FlashStage::Error` never constructed), src/update_ui.rs:118-133 (button gate: `has_fw_url` is false whenever `firmware_current.is_none()`, so the `"(Ensure device is connected)"` label and the disconnected-device flash path in start_firmware_update are unreachable from the UI).
- WHY: `cargo check` warnings; unreachable branches mask intent (same class as I5).
- FIX: Drop `latest` field (check_updates callers already hold the ReleaseInfo), remove or use `FlashStage::Error`, and either allow flashing with device disconnected (variant picker) or delete the dead label/path.
