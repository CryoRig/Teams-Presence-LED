# TPL01B - Teams Presence Lamp

Open `TPL01B.kicad_pro` in KiCad 10, then open the schematic and PCB
from the project manager's tree. This is a routed prototype design (rev B), not
an approved manufacturing release. The firmware and hardware release gates below
must be completed before ordering an assembled production batch.

## Board

- 40 mm circular outline, two copper layers, nominal 0.8 mm FR4.
- Eight front-facing WS2812B-B-V6 5050 RGB LEDs at 45-degree intervals on a
	10.5 mm radius. The LED ring is centered on the board.
- ESP32-S3-MINI-1-N8 on the rear: 8 MB quad-SPI flash, no PSRAM, native USB,
	and an integrated Wi-Fi/Bluetooth antenna. A module avoids a custom crystal
	and RF matching circuit and is smaller than the WROOM alternative.
- Rear right-angle USB-C receptacle at the 6-o'clock edge; the cable exits
	sideways, parallel to the PCB, rather than above or below the lamp.
- Rear BOOT and RESET switches, accessible without relying on working firmware.
- SMD components throughout, except the explicitly approved USB shield anchors.
	Ground stitching and signal vias are ordinary PCB features, not THT parts.
- A rule area keeps tracks, vias, pads and pours off both faces at the module
	antenna. The round outline is kept, so FR4 remains under the antenna; this is
	an accepted limitation. RF performance has not been measured.

The 40 mm diameter is the selected prototype size, not a proof of the minimum
possible diameter. No mounting holes or enclosure mounting geometry are defined.
Provide mechanical cable strain relief; the thin PCB and USB socket must not
carry an unsupported cable load in the finished enclosure.

## Circuit

USB-C has independent 5.1 kohm CC pull-downs, both USB2 plug orientations wired,
USBLC6-2SC6 ESD protection, and 22 ohm series resistors near the ESP32 USB pins.
The shield connects directly to ground. There is no USB-to-UART converter or
USB Power Delivery controller.

A resettable fuse feeds the 5 V rail. An AP63203 buck regulator supplies 3.3 V;
a TPS22918 controlled-rise-time switch supplies the LEDs and their 5 V
AHCT level shifter. Per-LED decoupling and local bulk/IC capacitors are fitted;
C6 (100 nF) and C18 (22 uF, added in rev B) sit beside the module supply pin.
The switch defaults off through its enable pull-down. The fuse is fault
protection, not current negotiation or a guarantee of USB compliance.

| Function | ESP32-S3 GPIO |
| --- | --- |
| LED data | 2 |
| LED supply enable, active high | 4 |
| Native USB D- / D+ | 19 / 20 |
| BOOT, active low | 0 |
| Download-mode strap pull-down | 46 |
| UART test pads TX / RX | 43 / 44 |
| RESET | EN, not a GPIO |

Recovery procedure: hold BOOT, press and release RESET, then release BOOT.
This selects the chip's ROM downloader independently of the application. No
eFuse changes or alternative software recovery GPIO scheme are needed.

## Sourcing

All 47 populated parts have MPN/LCSC fields in the rev B schematic; the passive,
fuse and inductor identities and their review status come from
[ORDERING.md](ORDERING.md). Availability of the major parts below was
checked against the JLCPCB snapshot and live listings on 2026-11-09/10; the
snapshot omitted some parts that the live catalog carried. Stock is not reserved.

| References | Manufacturer Part | LCSC/JLCPCB |
| --- | --- | --- |
| U1 | ESP32-S3-MINI-1-N8 | C2913206 |
| J1 | TYPE-C-31-M-12 | C165948 |
| D2-D9 | WS2812B-B-V6 | C52941391 |
| U2 | AP63203WU-7 | C780769 |
| U3 | SN74AHCT1G125DBVR | C7484 |
| U4 | TPS22918DBVR | C131941 |
| D1 | USBLC6-2SC6 | C7519 |
| SW1, SW2 | B3U-1000P | C231329 |

`LampParts.kicad_sym`, `LampParts.pretty`, `LampParts.3dshapes` and the local
library tables supply the imported module and checked right-angle connector model.
The connector locator holes are NPTH; its soldered shield anchors are plated.
Module ground-pad numbering was checked against the shared MINI land pattern.
The connector uses the native KiCad HRO M12 land pattern and the matching supplier
STEP geometry in a project-local footprint. OpenCascade measurements of the
model's locator pegs and seating plane were compared with the manufacturer
drawing. Its 180-degree Z rotation and -1.05 mm model-Y offset align the pegs
with the footprint's locator centres at (+/-2.89, -2.60) mm. The 8.94 mm body
width and 3.25 mm model height agree with the drawing. The PCB's locator holes
are 0.65 mm; the four shield slots match the manufacturer's layout.

J1 is at (100, 116.9) mm on the rear, with its opening facing outwards and its
mouth slightly overhanging the circular edge. Native DRC checks the shield
tails against the opposite-face LED courtyards. Front, rear and side renders
were inspected after replacement. Physical assembly, plug and enclosure checks
are still required; a 3D rendering is not a mechanical qualification.

Smaller MINI-V6 and MINI-V3J LEDs were rejected as unqualified drop-in replacements:
their retrieved datasheets require a longer T1L than the installed XIAO driver's
375 ns default. The LEDs were WS2812B-B/W (C114586) until 2026-11-12, when
JLCPCB stopped offering that part for assembly. They are now WS2812B-B-V6
(C52941391): same maker, same 5050 land pattern and pin positions, and a data
timing table identical to the V5 one (T0H 220-380 ns, T1H/T0L/T1L 580-1000 ns,
reset above 280 us). The separate custom-board firmware keeps FastLED's V5
profile unchanged; the existing XIAO build keeps its original generic timing to
preserve the currently supported SK6812 hardware. Validate the actual LEDs and
waveform during prototype bring-up; do not silently substitute LED revisions.

## Firmware And Release Gates

**Do not flash the existing XIAO image and assume the lamp will work.** The
separate `tpl01_rev_b` PlatformIO target is configured for the 40 mm
ESP32-S3-MINI-1-N8 board (hardware variant 2), with GPIO4 LED-rail gating, a
provisional 250 mA FastLED LED budget and the WS2812B-V5 timing profile, which
also satisfies the selected WS2812B-B-V6 (C52941391). Startup does not transmit LED data until USB is
configured and the switched rail is enabled. The target compiles but has not
been validated on a fabricated board. The bridge updater still offers the
published XIAO image only for variant 1; initial 40 mm programming uses the
matching image and physical BOOT/RESET recovery. The existing XIAO target and
its 480 mA LED setting remain unchanged.

See [the prototype-order checklist](ORDERING.md) for whole-board USB power,
assembly sourcing, manufacturing and hardware-validation gates.

The current 480 mA FastLED limit covers LEDs only, not the ESP32 or regulator.
Before enabling LEDs, establish the total startup, configured, suspend and
future-radio power budgets, USB descriptor/current policy, and switch timing.
CC resistors alone do not authorize a 1.5 A or 3 A load. Verify input inrush,
rail droop, brownout behavior, LED data when unpowered, and regulator temperature.

Rev B routes both USB data lines on the rear only, without vias: from J1 through
the USBLC6 pads, the 22 ohm resistors and into U1, with D+ on the x=99.05 column
and D- on the x=100.95 column. Copper length between pad centres is 9.7 / 9.9 mm
(D+ / D-) for the B6/B7 plug orientation and 10.1 / 12.5 mm for A6/A7; the
2.4 mm difference is the bridge joining the two D- contacts around a D+ contact.
Impedance is deliberately not controlled: the tracks are 0.2 mm wide on 0.8 mm
two-layer FR4 and the front copper beneath them is crossed by LED supply tracks.
That is accepted for a 10 mm full-speed (12 Mbit/s) link; test both cable
orientations on the first boards.

The bootstrap capacitor C3 sits at U2 pins 5/6 beside the inductor's switch pad,
so BUCK_SW and BUCK_BST use no vias. C2 sits at U2 pins 3/4 with a direct ground
track and two ground vias outside the lands; the former via inside the pin 4
land is gone. The 3.3 V feedback runs from C4 to U2 pin 1 around C3. Ripple,
startup, load steps, temperature and EMI are unmeasured.

Initial assembly assumes hot air/reflow and a stencil, not a soldering iron
alone. Confirm the exact footprint drawings, connector/PCB thickness, solder
paste apertures, via-adjacent solder pads, moisture handling and reflow order.
Check every assembly rotation in the supplier preview before placing a PCBA
order. No manufacturing Gerbers/drills are approved here.

## Validation

On 2026-11-11, KiCad 10.0.8 native checks on rev B reported:

- Schematic ERC: zero reported violations.
- PCB DRC after zone refill: zero violations, zero unconnected items and zero
	schematic-parity issues, using the project's configured check set.
- Independent schematic-to-PCB audit: all 173 intended pin/net assignments match;
	51 physical footprints, two copper layers, 40 mm outline, eight equally spaced
	front LEDs, and rear controller/USB/recovery controls.
- `tools/rev_b.py --verify` with the former rev A reference: USB data copper
	via-free on the rear, no via centre in an SMD land, outline, antenna rule area
	and LED/J1/U1/switch placements identical to rev A. Two ground vias still
	overlap a land with their annular ring only (C8 pad 2 by 0.225 mm, C16 pad 2
	by 0.09 mm); they were not changed.
- Front/rear copper inspection outputs generated under ignored `outputs/`.

The fabrication floor is 0.15 mm track width with 0.15 mm default clearance,
0.20 mm power clearance and 0.50 mm copper-edge clearance. Wider power routes
use the Power netclass, with short pad escapes where necessary. Confirm all
limits with the selected fabrication/assembly service; DRC is not an electrical,
thermal, RF or manufacturability certification.

Hardware testing is **not run**: no prototype of this PCB has been fabricated.
The existing bench lamps have not been flashed or used as proof of this design.

Use the development helpers only with a dedicated Python virtual environment
that can import the matching KiCad `pcbnew` bindings. For a read-only audit:

```text
python pcb/tools/check_board.py --netlist pcb/outputs/rev-b/netlist.xml --routing --connector
python pcb/tools/rev_b.py --verify
```

Export that XML from the current schematic first; generated outputs are not
included in the repository. `--connector` checks the specific M12 footprint,
grounded shield slots, locator holes and STEP transform. `rev_b.py --verify`
checks the current board without a reference; supply `--reference <board>` to
also compare the outline, antenna keepout and fixed placements with an earlier
board. The former rev A snapshot is not included.
`rev_b.py --schematic` and `--board` are the one-time record of the rev A to
rev B changes and refuse to run twice. `set_part.py` changes the Value, MPN,
LCSC and Datasheet fields of existing parts in schematic and board. Other helper switches perform
development layout/route operations and can replace routing; do not run them on
an edited board as a routine validation step.

The JLCPCB placement file is not the native KiCad position file. JLCPCB places
the centre of its own library footprint and reads bottom-side angles mirrored,
so `jlc_placement.py <review folder> --fetch` (easyeda2kicad environment)
downloads those footprints and `--fit` (pcbnew environment) fits each one to
the board pads and writes `jlc-placement.csv`, which `export_jlc_review.ps1`
turns into the CPL. Neither step modifies the board.

Freerouting's Windows native CLI 2.5.0 failed with missing GraalVM serialization
metadata for `ComponentOutline`. Routing used the official 2.5.0 JAR and an
isolated Java 25 runtime instead, with verified download SHA-256 values. Native
KiCad DRC, not Freerouting's completion score, is the final connectivity check.

## Copilot Connection

The local connector uses KiCAD-MCP-Server v2.8.3, installed outside this repository
in the sibling `KiCAD-MCP-Server/2.8.3` directory. Its Python dependencies are
isolated in that installation's `.venv`. The computer-specific VS Code MCP
configuration is in `.vscode/mcp.json`, which the repository already ignores.

Enable the KiCad API under Preferences > Preferences > Plugins. For live PCB
editing, open the PCB from the project manager rather than launching a separate
PCB Editor process: separate processes competed for the default Windows API
endpoint during setup.

Before edits, use `open_project` with this project's absolute path and check
`get_backend_state`. Expect `backend: ipc`, `realtime: true`, and this board's
path. File-based schematic edits require saving and closing the schematic editor
first, then reopening it afterward; do not edit the same document simultaneously.

Keep schematic, PCB, project, custom library, and design-rule source files under
version control. Local state, lock files, backups, and the `outputs/` directory
are ignored. Generate manufacturing files only after circuit review and ERC/DRC.