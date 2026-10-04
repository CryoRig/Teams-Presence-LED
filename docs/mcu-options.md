# Alternative MCU Candidates

This is a reference shortlist, not a list of tested firmware targets. The current
implementation targets the Seeed XIAO ESP32-S3 using PlatformIO, Arduino, and
FastLED. For this project, native USB **device** support matters more than CPU
speed: the bridge talks to a vendor-defined HID interface, and the firmware
drives eight WS2812B LEDs and stores LED calibration persistently.

## Off-the-shelf development boards

These are alternative MCUs with familiar development boards and a PlatformIO /
Arduino route. Board availability and price depend on supplier and region.

| MCU | Example board | Assessment |
| --- | --- | --- |
| ESP32-S2 | LOLIN S2 Mini | Closest software relative to the current ESP32-S3; native USB and often inexpensive boards. First candidate for a lower-effort port. |
| RP2040 | Raspberry Pi Pico | Strong low-cost option with native USB and suitable LED output; custom HID needs a USB-stack port. |
| SAMD21G18A | Seeed XIAO SAMD21 | Small, familiar form factor, native USB, and enough capacity for eight LEDs. |
| STM32F072CB | STM32F072 development board | Cost-conscious STM32 with native USB; expect additional HID integration work. |
| STM32F411CE | WeAct Black Pill | Common, inexpensive board with native USB; more processing capacity than needed. |
| ATmega32U4 | Pro Micro | Familiar native-USB Arduino option with EEPROM; 2.5 KB RAM makes this a tight port. |
| SAMD51J19A | Feather M4 Express | USB-capable Arduino family, but likely more board cost and power than needed. |
| nRF52840 | Feather nRF52840 | USB and Arduino support, but Bluetooth adds cost without helping this wired design. |

**First to try:** ESP32-S2 for minimal software disruption; RP2040 if board
cost is the priority. PlatformIO board support does not mean the current firmware
will compile by changing `board` in `platformio.ini`.

## Custom PCB / bare MCU

For a custom board, compare the whole circuit and assembly cost, not just the
MCU or a development board's price. Check external flash, clocking, USB
connection, power circuitry, programming/recovery access, and component
availability for the chosen package. The ordering here is a prototyping
priority, not a verified bill-of-materials ranking.

| MCU | Custom-board assessment |
| --- | --- |
| RP2040 | Best first overall candidate: widely used, inexpensive, native USB; requires external flash and a custom HID port. |
| SAMD21G18A / E18A | Compact conventional choice with native USB and sufficient capacity; choose the package to suit the layout. |
| STM32F072CBT6 | Good cost-conscious STM32 candidate with native USB and comfortable memory; HID needs a port. |
| ESP32-S2 | Least disruptive path from the present ESP32 firmware, but its wireless hardware is unnecessary for a wired indicator. |
| STM32F042K6T6 | Small potential low-cost option; 32 KB flash and 6 KB RAM make firmware size and USB feasibility a measure-first question. |
| ATmega32U4 | Native USB and EEPROM; limited RAM/flash require a leaner firmware and careful USB/LED timing. |
| STM32F411CEU6 | Plenty of headroom, but likely more MCU than this design needs. |
| RP2350 | Technically suitable, but start with RP2040 unless testing shows a reason to move up; check PlatformIO/core support for the selected board. |

**First to prototype:** RP2040 and SAMD21, using ESP32-S2 as the
low-porting-effort comparison. SAMD51 and nRF52840 drop out of the custom-board
shortlist because their extra capabilities are unlikely to repay their cost here.

## Compatibility checks before selecting hardware

- The bridge expects a HID device with usage page `0xFF00`, report ID `0x06`,
  and six-byte input/output reports (report ID plus five payload bytes). Match
  these descriptors and reports, or update the bridge and protocol together.
- `UsbManager` currently uses the ESP32 Arduino `USBHIDVendor` API and a
  FreeRTOS queue. Other families need a USB implementation and replacement
  queue/dispatch mechanism; `Preferences` calibration storage also needs a
  platform-specific equivalent.
- FastLED must drive the eight WS2812B LEDs without upsetting USB timing.
  Verify the intended LED supply and signal voltage on the actual board.
- The bridge's automated firmware flashing and version handling currently
  target the ESP32-S3. A new target needs an explicit upload, recovery, and
  firmware-update plan; PlatformIO build support alone does not provide one.
- For an RP2040 Arduino-Pico/TinyUSB approach, the documented PlatformIO setup
  uses a separate platform integration; verify it before treating this as a
  drop-in PlatformIO target. RP2350 support also depends on the selected core.
- Before committing a PCB, test enumeration and HID round trips with the
  bridge, all eight LEDs, persistent calibration, and recovery from a bad flash.

Avoid substituting an ESP32-C3 or ESP32-C6 solely because it has a USB port:
its USB Serial/JTAG interface is not the general-purpose USB device controller
needed by the current custom HID design.

## Platform references

- [Espressif 32](https://docs.platformio.org/en/latest/platforms/espressif32.html)
- [Raspberry Pi RP2040](https://docs.platformio.org/en/latest/platforms/raspberrypi.html) and [Arduino-Pico PlatformIO integration](https://arduino-pico.readthedocs.io/en/latest/platformio.html)
- [Atmel SAM](https://docs.platformio.org/en/latest/platforms/atmelsam.html)
- [ST STM32](https://docs.platformio.org/en/latest/platforms/ststm32.html)
- [Atmel AVR](https://docs.platformio.org/en/latest/platforms/atmelavr.html)
- [Nordic nRF52](https://docs.platformio.org/en/latest/platforms/nordicnrf52.html)