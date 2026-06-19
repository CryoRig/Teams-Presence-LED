# Teams Presence USB LED Indicator

This project consists of two independent sub-projects in the same repository: a C# bridge application and ESP32 firmware.

## Phase 1 — Repository Structure

Set up a monorepo with three top-level folders:

- [bridge/](bridge/) — C# .NET solution
- [firmware/](firmware/) — PlatformIO ESP32 project
- [docs/](docs/) — protocol specification and wiring notes

## Phase 2 — Serial Protocol

Define a simple newline-terminated ASCII protocol so both sides can be developed independently.

| Command | Meaning |
| --- | --- |
| `SOLID:R,G,B\n` | Set all LEDs to a solid color |
| `BREATHE:R,G,B\n` | Start a breathing/pulsing animation |
| `OFF\n` | Turn all LEDs off |
| `PING\n` | Bridge heartbeat; the ESP32 replies with `PONG\n` |

This protocol is documented in [docs/protocol.md](docs/protocol.md) and serves as the shared reference for both implementations.

## Phase 3 — Debug and Interaction Tooling

Before writing any firmware logic, the debug and serial interaction layer must be set up and verified. This phase is a prerequisite for Phase 4.

### Board selection (locked here)

Use the **ESP32-S3-DevKitC-1**. The ESP32-S3 has a built-in USB Serial/JTAG controller (GPIO19 D−, GPIO20 D+) that exposes both a CDC-ACM serial port and a JTAG debug interface over a single USB cable simultaneously. No external probe or FTDI/CH340 bridge chip is needed.

### PlatformIO debug configuration

Add to `firmware/platformio.ini`:

- `debug_tool = esp-builtin` — uses the on-chip JTAG via OpenOCD
- `upload_protocol = esp-builtin` — flash over the same USB port
- `monitor_speed = 115200`
- `monitor_filters = send_on_enter` — buffers keyboard input and sends a full line on Enter, matching the `\n`-terminated protocol
- `monitor_echo = yes` — shows typed commands in the terminal
- `monitor_eol = LF` — ensures `\n` is sent, not `\r\n`

### Windows driver setup (one-time)

The Espressif USB JTAG driver must be installed before OpenOCD can claim the JTAG interface. Without it, PlatformIO reports `LIBUSB_ERROR_NOT_FOUND`. Install via the Espressif Installation Manager or with `idf-env.exe --driver install --espressif` in PowerShell. Document this step in `docs/setup.md`.

### Serial test script

Create `tools/serial_test.py` — a minimal Python script using `pyserial` that:

- Accepts a COM port and baud rate as arguments
- Connects to the ESP32 CDC serial port
- Accepts commands typed on stdin and sends them as `\n`-terminated lines
- Prints responses with timestamps

This script is the primary interaction method during firmware development: run it in one terminal to send protocol commands such as `SOLID:0,255,0` or `PING` and read the ESP32's responses, while the PlatformIO debugger can be attached simultaneously in VS Code.

### Verification gate

Before moving to Phase 4, confirm all three of the following:

- The ESP32-S3 board appears as a COM port and a simple `Serial.println("BOOT")` sketch is visible in `pio device monitor`
- The PlatformIO debugger attaches (`debug_tool = esp-builtin`), a breakpoint in `setup()` is hit, and step-through works
- `tools/serial_test.py` sends a line and receives a reply on the same USB connection while the debugger is idle

## Phase 4 — ESP32 Firmware

- Toolchain: PlatformIO with the Arduino framework
- Key libraries: FastLED for WS2812B LEDs, with no WiFi stack required

### Firmware behavior

- Listen on serial at 115200 baud and parse lines terminated by `\n`
- Map commands such as `SOLID`, `BREATHE`, `OFF`, and `PING` to LED actions
- Use a FastLED animation engine for solid fill and sine-wave breathing with non-blocking timing via `millis()`
- Run a heartbeat watchdog: if no `PING` is received within 30 seconds, enter a disconnected state with a slow white pulse
- Show a quick boot animation to confirm that the device is alive

### Presence-to-color mapping

| Teams status | Command sent | Visual |
| --- | --- | --- |
| Available | `SOLID:0,200,0` | Green solid |
| Busy | `SOLID:200,0,0` | Red solid |
| DoNotDisturb | `SOLID:200,0,0` | Red solid (full brightness) |
| Away | `BREATHE:255,120,0` | Amber breathing |
| BeRightBack | `BREATHE:255,80,0` | Orange slow pulse |
| Offline | `OFF` | Off |
| Unknown | `BREATHE:80,80,80` | Grey slow pulse |

## Phase 5 — Bridge App

Target: a .NET 8 console application, with the option to evolve into a Windows tray app later.

### Teams API discovery

- Check the registry at `HKCU\Software\Microsoft\Office\Teams`
- Fall back to scanning known ports such as 8124 and 8125
- Reference the community-documented endpoint `http://localhost:{port}/presenceState`
- Note that this API is used by certified headset vendors such as Jabra and Plantronics, though it remains undocumented and should be guarded by version checks

### Bridge behavior

- Poll the Teams presence endpoint every 5–10 seconds
- Compare the latest state with the previous one to avoid redundant serial writes
- Use a config-driven mapping dictionary from presence state to commands, stored in a JSON file so colors can be remapped without recompiling
- Manage the serial port with `System.IO.Ports.SerialPort`
  - Auto-detect the ESP32 COM port by VID/PID via USB descriptor
  - Auto-reconnect if the device is unplugged and replugged
  - Send `PING` every 15 seconds as a heartbeat
- Document Task Scheduler setup to run the bridge on Windows login in a hidden or minimized mode

## Phase 6 — Integration and Testing

- Flash the firmware and open a serial monitor
- Manually send `SOLID:0,255,0\n` via `tools/serial_test.py` to verify the LED response
- Run the bridge while Teams is open to verify that status changes drive the LED correctly
- Test USB disconnect and reconnect to confirm auto-reconnect behavior
- Test Teams going offline to verify that the system shows `OFF` or a grey pulse
- Test bridge crashes or restarts to confirm that the ESP32 watchdog kicks in after 30 seconds

## Relevant Files

- [docs/protocol.md](docs/protocol.md) — serial protocol spec
- [docs/setup.md](docs/setup.md) — one-time Windows driver and toolchain setup
- [firmware/src/main.cpp](firmware/src/main.cpp) — ESP32 entry point
- [firmware/platformio.ini](firmware/platformio.ini) — board and library configuration
- [tools/serial_test.py](tools/serial_test.py) — interactive serial test script (pyserial)
- [bridge/TeamsPresenceBridge/Program.cs](bridge/TeamsPresenceBridge/Program.cs) — bridge entry point
- [bridge/TeamsPresenceBridge/TeamsClient.cs](bridge/TeamsPresenceBridge/TeamsClient.cs) — local Teams API polling
- [bridge/TeamsPresenceBridge/SerialManager.cs](bridge/TeamsPresenceBridge/SerialManager.cs) — serial port handler
- [bridge/TeamsPresenceBridge/config.json](bridge/TeamsPresenceBridge/config.json) — presence-to-color mapping

## Decisions and Scope Boundaries

- Board locked to **ESP32-S3-DevKitC-1** for native USB JTAG; the entire debug strategy depends on this choice
- No cloud and no Microsoft Graph; the solution is fully local and works offline
- USB serial only; no WiFi is required on the ESP32, keeping the firmware simpler
- No Teams app registration; the bridge relies on the local Teams API and should fail gracefully with a clear error if it becomes unavailable
- Out of scope: multi-user support, mobile Teams, and macOS/Linux bridge support

## Further Considerations

- Teams API stability is a concern because the local endpoint is undocumented and has changed between Teams versions; adding a version handshake and clear logging would make updates easier
- The Windows Espressif USB JTAG driver is a one-time manual prerequisite that must be installed before Phase 3 work can begin