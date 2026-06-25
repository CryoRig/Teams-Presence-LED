# Serial Protocol Specification

This document defines the newline-terminated ASCII protocol used for communication between the **Teams Presence Bridge** (C#) and the **LED Indicator** (ESP32).

## Communication Parameters

| Parameter | Value |
| :--- | :--- |
| **Baud Rate** | 115200 |
| **Data Bits** | 8 |
| **Stop Bits** | 1 |
| **Parity** | None |
| **Line Terminator** | `\n` (LF) |

## Command Set

All commands must be terminated with a newline character (`\n`).

### 1. Set Solid Color
Sets all LEDs in the chain to a single, static color.
- **Format:** `SOLID:R,G,B\n`
- **Arguments:**
  - `R`: Red component (0-255)
  - `G`: Green component (0-255)
  - `B`: Blue component (0-255)

### 2. Start Breathing Animation
Starts a continuous "breathing" (pulsing) effect using the specified color with a moderate speed (~3 second cycle).
- **Format:** `BREATHE:R,G,B\n`
- **Arguments:**
  - `R`: Red component (0-255)
  - `G`: Green component (0-255)
  - `B`: Blue component (0-255)

### 3. Start Slow Breathing Animation
Starts a continuous "breathing" (pulsing) effect using the specified color with a slow speed (~5 second cycle). Intended for away/idle states.
- **Format:** `BREATHE_SLOW:R,G,B\n`
- **Arguments:**
  - `R`: Red component (0-255)
  - `G`: Green component (0-255)
  - `B`: Blue component (0-255)

### 4. Turn Off
Turns all LEDs off immediately.
- **Format:** `OFF\n`

### 5. Heartbeat / Ping
Used by the Bridge to verify that the ESP32 is still connected and responsive.
- **Format (Bridge to ESP32):** `PING\n`
- **Response (ESP32 to Bridge):** `PONG\n`

## Error Handling & Edge Cases

1. **Malformed Commands:** If the ESP32 receives a command that does not match any of the above formats, it should ignore the command and remain in its current state.
2. **Loss of Communication:** If the ESP32 does not receive a `PING` command for **30 seconds**, it must enter "Disconnected State" (a moderate white pulse) to alert the user.
3. **Invalid RGB Values:** Any value outside the 0-255 range should be treated as an error and ignored.