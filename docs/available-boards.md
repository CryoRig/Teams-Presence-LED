# Boards on Hand

These PlatformIO targets compile a serial version of the LED firmware for the
boards listed below. Only the Seeed XIAO ESP32-S3 is currently supported over
custom USB HID. The new targets use 115200-baud serial instead, even on boards
with native USB. **A successful build does not confirm that an untested board
has the expected pinout, bootloader, or reliable LED timing.**

| Available board / identified chip | PlatformIO environment | Connection to the Windows bridge | LED output pin |
| --- | --- | --- | --- |
| ESP-01, ESP8266EX with T25S80 (1 MB) flash | `esp01_1m` | External **3.3 V** USB-to-UART adapter | GPIO2 (boot strap; keep high at reset) |
| Seeed XIAO RP2040 | `xiao_rp2040` | Built-in USB serial | GPIO2, labeled D8 (not D2) |
| STM32F030 demo v1.2, STM32F030F4P6 | `demo_f030f4` | External **3.3 V** USB-to-UART adapter | PA7 |
| Nano clone, ATmega168PA | `nano_atmega168` | On-board USB-to-serial converter | D2 |
| Uno clone, ATmega328P-PU | `uno_atmega328` | On-board USB-to-serial converter | D2 |
| Pro Mini clone, ATmega328P-MU | `pro_mini_atmega328` | External USB-to-UART adapter | D2 |
| Nano, ATmega328P | `nano_atmega328` | On-board USB-to-serial converter | D2 |
| Pro Micro, ATmega32U4 | `pro_micro_atmega32u4` | Built-in USB CDC serial | D2 |
| LuatOS Core ESP32-C3, 4 MB flash | `luatos_esp32c3` | On-board CH343 USB-to-UART (UART0) | IO04 / GPIO4 |
| Raspberry Pi Pico, RP2040 (RP2-B1) | `rpi_pico` | Built-in USB serial | GPIO2 |

These are example target definitions based on the named MCU, **not verified
board revisions**. In particular, check the actual Pro Mini and Pro Micro
voltage/clock before flashing: the targets above assume **5 V / 16 MHz**.
For a 3.3 V / 8 MHz Pro Mini choose `pro8MHzatmega328`, and for a 3.3 V /
8 MHz Pro Micro choose `sparkfun_promicro8`; add matching environments or
change the board locally. The ATmega328 Nano may use an old or new bootloader;
if uploading fails, try changing its board to `nanoatmega328new`. Confirm that
the ESP-01 flash is actually 1 MB before flashing the 1 MB target. The tested
LuatOS C3 board has a CH343 adapter on its USB connector, wired for UART0;
the target uses DIO flash mode as required by the board's external flash.

## Build and connect

1. From the repository root, build one target with `pio run -d firmware -e
   rpi_pico` (replace `rpi_pico` with the environment name above). To upload,
   use `pio run -d firmware -e rpi_pico -t upload`; a board may require its
   bootloader button, the correct COM port, or an external programmer.
2. Connect eight WS2812B LEDs with the chosen LED pin to the first LED's DIN
   and a **common ground** between the board and LEDs. Supply the LEDs from an
   appropriate 5 V source, not a 3.3 V MCU output pin. Budget up to 480 mA
   for eight LEDs at full white; test power/USB limits and use an appropriate
   logic level shifter if 3.3 V data is unreliable at 5 V LED supply. The
   F030's small NeoPixel driver does not enforce the S3 FastLED power cap.
3. For UART boards, cross TX/RX and connect ground; use the MCU's 3.3 V UART
   levels where required. A USB connector on an Uno/Nano-style board reaches
   its UART through a USB-to-serial converter. A Pro Mini has no USB connector.
   Confirm the STM32 board's `Serial` UART pins against its schematic before
   wiring the adapter; the demo board's USB/power connection is not the
   project's HID or serial transport.
4. Close any PlatformIO serial monitor. In the bridge's `config.json` next to
   the bridge executable, set `"serialPort": "COM9"` (replace `COM9` with your
   device's port). Restart the bridge. The setting is opt-in: without it the
   original ESP32-S3 HID path remains selected. The bridge only opens that
   specified port and requires a compatible version reply before connecting.
   For RP2040 and Pro Micro, this is **CDC serial, not vendor HID**.
5. To return to the original S3, remove `serialPort` or set it to `null` and
   restart the bridge. The S3 build remains `pio run -d firmware -e
   seeed_xiao_esp32s3`.

For the tested ESP-01 adapter, unplug USB before moving its switch: select
`PROG` for flashing, then select `UART` and reconnect to run the firmware.
The PlatformIO-bundled esptool stalled while loading its RAM stub. With the
ESP8266-compatible newer esptool installed locally, this no-stub upload worked
at 115200 baud (run from the repository root after building `esp01_1m`):

```powershell
python "$HOME\.platformio\packages\tool-esptoolpy\esptool.py" --chip esp8266 --port COM24 --baud 115200 --no-stub write_flash --flash_mode qio --flash_freq 40m --flash_size 1MB 0x0 firmware/.pio/build/esp01_1m/firmware.bin
```

Replace `COM24` with the actual adapter port. Keep the strip disconnected
unless GPIO2/DIN has a secure connection that does not pull GPIO2 low at boot.

## Current limitations

- The serial link uses fixed seven-byte frames: sync `0xA5`, five protocol
  bytes, and the XOR of the preceding six bytes. The five data bytes carry
  the same command or response payloads as the HID protocol, without the HID
  report ID. An input frame with version variant `2` identifies this serial
  firmware; variant `1` remains the S3 HID firmware.
- The serial firmware supports presence commands, brightness, transition
  duration, version, ping, and the disconnected animation. LED calibration,
  calibration test, and the bridge's automated firmware updater are **not
  supported** on these boards. Flash them with PlatformIO instead.
- The STM32F030F4 build occupies almost all 16 KB of flash. Treat it as
   experimental: a bootloader, different core, or additional features could
   make it too large. Its UART pinout, upload, LED timing, and recovery have not
   yet been tested on hardware.
- ESP-01 needs correct boot strapping and a stable external 3.3 V supply;
   the ESP32-C3's native USB Serial/JTAG is separate from this board's CH343
   UART and does **not** expose the existing vendor HID interface.

## Live-test record

| Board | Result |
| --- | --- |
| Raspberry Pi Pico (`rpi_pico`) | UF2 uploaded via BOOTSEL; enumerated on Windows as USB CDC; version `0.6.2` variant `2` and PONG checksum validated; eight LEDs visibly entered watchdog mode, then displayed commanded dim red; OFF command and PONG succeeded. USB CDC requires DTR asserted on the host. |
| Seeed XIAO RP2040 (`xiao_rp2040`) | UF2 uploaded via BOOTSEL after 1200-baud reset; enumerated as USB CDC on COM11; version `0.6.2` variant `2` and PONG checksum validated; user confirmed dim red on D8/GPIO2; OFF command and PONG succeeded. |
| Arduino Uno ATmega328P (`uno_atmega328`) | Uploaded and flash-verified on COM7 (ATmega328P signature `0x1E950F`); version `0.6.2` variant `2` and PONG checksum validated after reset-on-open; user confirmed dim red on D2; OFF command and PONG succeeded. |
| Nano ATmega328P (`nano_atmega328`) | Uploaded and flash-verified on CH340 COM14/COM24 (ATmega328P signature `0x1E950F`). **Communication/LED test blocked:** on both its original USB socket (location `1-5`) and the ATmega168 Nano's known-good socket (`1-8`), Windows lists the adapter but .NET and pyserial cannot configure it (error 31). A serial-only diagnostic was uploaded and verified, but its periodic output could not be observed because the COM port still would not open. The corrected production firmware was restored and flash-verified afterward. This does not establish a firmware failure; investigate this board's CH340/Windows driver or use a different serial adapter. |
| Pro Micro ATmega32U4, 5 V / 16 MHz (`pro_micro_atmega32u4`) | Uploaded and flash-verified through its Caterina bootloader (ATmega32U4 signature `0x1E9587`); enumerated as USB CDC on COM22; version `0.6.2` variant `2` and PONG checksum validated; user confirmed dim red on D2; OFF command and PONG succeeded. |
| LuatOS Core ESP32-C3 (`luatos_esp32c3`) | **LED output failed:** firmware reflashed and hash-verified on CH343 COM23 using UART0 and DIO flash mode (ESP32-C3 revision v0.4), explicitly reset, and answered version `0.6.2` variant `2` and PONG. The external strip remained solid white after red-only at 25% brightness and after OFF, each followed by a valid PONG. FastLED logged that it selected a generic clockless fallback, not a C3-specific driver. Retest later with a different WS2812 driver such as Adafruit NeoPixel; do not count this board as fully working. |
| Nano ATmega168PA (`nano_atmega168`) | Uploaded and flash-verified on CH340 COM24 (ATmega168 signature `0x1E9406`). The original firmware did not answer commands; a serial-only diagnostic proved UART RX/TX and checksum handling worked. After production firmware stopped repeating unchanged FastLED transfers, VERSION `0.6.2` variant `2` and PONG succeeded, the user confirmed dim red on D2, and OFF and PONG succeeded. |
| Pro Mini ATmega328P (`pro_mini_atmega328`) | **Deferred:** no onboard USB or external USB-to-UART adapter currently available. No flash or live test attempted; confirm board voltage/clock and obtain a matching adapter before testing. |
| ESP-01 ESP8266EX (`esp01_1m`) | **Flash and serial passed; LED output untested.** The CH340C adapter supplied 3.2-3.3 V at VCC/RX; esptool identified ESP8266EX and 1 MB flash on COM24. The PlatformIO-bundled esptool stalled loading its RAM stub, but the newer esptool wrote the image in no-stub mode. In UART mode the firmware returned version `0.6.2` variant `2` and valid PONG. GPIO2 could not be connected securely to the external strip while seated in the adapter; do not use hand-held wires to test it. |

The other alternative boards in this guide have only been build-tested so far.