# TPL01B - 40 mm JLCPCB Prototype Order Checklist

## Confirmed Requirements

Confirmed by the user on 2026-11-10:

- Fully assembled boards, including both faces, LEDs and USB sockets.
- Start with JLCPCB's minimum assembled quantity; cost optimization is not a goal.
- Power comes from an ordinary PC USB-A port through an A-to-C cable.
- Plastic base and replaceable diffuser; no metal or magnets planned. The
  enclosure can be adapted to the PCB dimensions.
- Use the 40 mm two-layer board with ESP32-S3-MINI-1-N8, eight WS2812B LEDs
  (WS2812B-B-V6 since 2026-11-12),
  sideways USB-C, and physical BOOT/RESET. Do not change the protected PCB layout
  unless review establishes a specific hardware or assembly flaw.

## Current Release Status

Rev B (2026-11-11) reworks the USB and regulator layouts inside two
small rear-side areas and adds one capacitor. On 2026-11-12
the LEDs changed to WS2812B-B-V6 (C52941391) and the placement file was
converted to JLCPCB's conventions; copper and placement are unchanged. Native ERC,
DRC and schematic parity are clean and the independent net/connector audit
passes. **It is not order-approved yet:** supplier DFM and assembly preview are
still required; functional hardware validation follows prototype assembly.
Retain the existing USB connector unless JLCPCB raises an assembly issue
during order review.

The current project is rev B with the new LED identity. Earlier local snapshots
and review exports are not included. No 40 mm hardware has been fabricated or
physically tested.

## Supplier Review Package

Generate a fresh supplier review package from the current design before seeking
JLCPCB feedback; previous exports are not included. Gerbers, separated
plated/nonplated drills, BOM and CPL are for feedback only, not automatic
fabrication approval. BOM and CPL should cover 47 populated positions in 21
groups. The schematic carries every LCSC/MPN identity; the export helper checks
each one against the table below. Fit the CPL to JLCPCB's footprints and check
all component rotations and positions in the supplier preview.

## PC USB Power

FastLED's 480 mA setting is an estimate for LED consumption, not a measured
limit on total USB input current. It does not include the ESP32, conversion
losses, buffers or startup current, and is not an overcurrent protection circuit.

The `tpl01_rev_b` prototype firmware allocates 250 mA to LEDs, approved by the
user on 2026-11-11 for initial prototypes,
leaving 250 mA of the configured 500 mA USB budget for electronics, estimation
error and transient headroom. This allocation must be checked on the actual
assembled boards, including maximum-white, startup and intended radio operation.
Only consider raising the LED budget after live measurements support it.
The existing XIAO firmware retains its original 480 mA LED setting.

A startup-path review found that setup sent a black FastLED frame while the
GPIO4-controlled LED/level-shifter rail was off. The custom target now
skips that transmission; first LED data is sent only after USB is mounted and the
rail is enabled. The data pin is driven low before the rail is switched off.
Both current PlatformIO targets build successfully after this change. This fixes
a credible unpowered-input/backfeed risk, but does not replace board bring-up.

The Worldsemi V5 datasheet for the original C114586 and the WS2812B-B-V6
datasheet for its replacement C52941391 both specify T0H 220-380 ns,
T1H/T0L/T1L 580-1000 ns, and reset low >280 us. FastLED's ESP32 RMT5 encoder
uses 25 ns ticks, so the custom target overrides the V5 profile to exact-tick
values: 250/975 ns for bit 0 and 600/625 ns for bit 1, with a 300 us reset.
The XIAO target keeps generic timing for its current SK6812 bench hardware.
This datasheet-based code configuration is not a measured waveform or hardware
operation test.

- `tpl01_rev_b`: TPL01B 40 mm ESP32-S3-MINI-1-N8 board, hardware variant 2, 8 MB flash.
- GPIO4 keeps the LED rail off before USB configuration and when suspended or
  disconnected. The custom configuration requests 500 mA, bus-powered.
- The custom 40 mm target and original XIAO target compile successfully.
- Suspend handling of the LED rail does not prove that the entire ESP32 board
  meets USB suspend-current limits. Whole-device low-power behavior remains a
  bring-up/review gate, not a compliance claim.
- No custom-target firmware has been flashed onto a 40 mm prototype PCB.
- The existing bridge updater only accepts hardware variant 1 for the published
  XIAO firmware. That restriction is intentionally retained; initial custom-board
  programming uses the matching build target and physical BOOT/RESET path.
- User decision on 2026-11-11: defer custom-board bridge support until assembled
  boards have passed basic bring-up tests. It is not a prerequisite for ordering.

## JLCPCB Service And Sourcing

The current JLCPCB capability page lists double-sided placement and packages
below 0402 under Standard PCBA. Standard PCBA requires rails/fiducials and lists
a 70 x 70 mm minimum single-board/panel handling size. This 40 mm circular board
therefore needs an agreed carrier/panel arrangement; request supplier
panelization and review depanelization locations, USB clearance and the module
antenna edge. Do not enlarge the finished PCB to meet panel size.

Live part pages checked on 2026-11-11; stock can change and is not reserved:

| Item | Part | Evidence | Required Action |
| --- | --- | --- | --- |
| 40 mm MCU | ESP32-S3-MINI-1-N8, C2913206 | 22,165 listed; Standard PCBA; X-ray required | Confirm available order quantity at checkout |
| LED ring | WS2812B-B-V6, C52941391 | 22,717 listed on 2026-11-12; Standard PCBA; MSL 5a; JLCPCB lists no datasheet (LCSC copy retained) | Replaces WS2812B-B/W C114586, no longer offered for assembly; confirm in the BOM upload, then reflow handling and timing |
| 40 mm USB | TYPE-C-31-M-12, C165948 | 282,855 listed; Economic/Standard PCBA | Confirm the exact right-angle footprint/mating orientation in JLC's assembly preview |
| 40 mm buck | AP63203WU-7, C780769 | 2,801 listed; Economic/Standard PCBA | Confirm exact MPN and inductor AVL |
| 40 mm LED buffer | SN74AHCT1G125DBVR, C7484 | 34,964 listed; SOT-23-5; Economic/Standard PCBA | Confirm reel quantity and placement package |
| LED rail switch | TPS22918DBVR, C131941 | 68,631 listed; Economic/Standard PCBA | Confirm reel quantity and placement package |
| USB ESD part | USBLC6-2SC6, C7519 | 24,774 listed; Economic/Standard PCBA | Confirm reel quantity and placement package |
| BOOT/RESET switches | B3U-1000P, C231329 | 77,593 listed; Economic/Standard PCBA | Confirm actuator alignment/height in the final diffuser/base |
| Passives, fuse and buck inductor | Identities in the rev B schematic, listed below | Not a released AVL/BOM | Qualify exact manufacturer part and current JLC match for each line |

J1 here is the retained 40 mm board's HRO TYPE-C-31-M-12, C165948, not the
connector from the retired compact board. Its parts listing specifies "SMT
Assembly". The footprint also has four plated through-hole `EH` shell anchors
(called `SH` on the actual PCB) in addition to the 12 SMT signal pads. The
actual native drill export uses four plated slots with a 0.60 mm routing tool,
not the earlier catalog pinout's 0.8 mm hole shorthand. On 2026-11-11 the
user chose to retain this connector and proceed to JLCPCB order review without
requiring separate advance confirmation of anchor soldering. Revisit only if
JLCPCB reports an assembly limitation; the listing is not confirmation of the
exact anchor-soldering process. No connector redesign or post-delivery hand
soldering is authorized by this decision.

JLCPCB documents pre-order/global sourcing and customer consignment. A search
page showing no result is not proof that a part cannot be sourced. Procurement
must be confirmed in the actual JLCPCB account; parts have not been reserved,
purchased or added to a private library by this review. The current overseas
consignment instructions describe a formal sale/import process, price review,
customs deposit and handling fee, minimum fee and supporting purchase records;
do not assume that shipping loose connectors to the PCBA line is sufficient.

Rev A's netlist had **30** populated components without LCSC identifiers
(excluding bare test pads): primarily passives, plus the resettable fuse and
buck inductor. Rev B writes the identities below into the schematic and adds
C18, giving 31 positions.

### Passive, Fuse And Inductor Identities

Quantities are per board. `tools/rev_b.py` copied this table into the rev B
schematic and `tools/export_jlc_review.ps1` checks the schematic against it.
Searches and live part-detail lookups on 2026-11-11 identified stocked
candidates for all positions: **24 straightforward catalog matches**,
**5 capacitors accepted for first prototypes** (C18 reuses the C4/C5 part),
**F1 engineering-reviewed for the prototype**,
and **L1 engineering-reviewed for the prototype**.
These are procurement candidates, not a released BOM; no footprint was changed
for them.

| References | Qty | Schematic Requirement | Existing Package | Candidate LCSC / MPN | Status |
| --- | --- | --- | --- | --- | --- |
| C1 | 1 | 1 uF, 16 V | 0603 | C29936 / Samsung CL10B105KA8NNNC | Catalog match: 1 uF, 25 V, X7R, +/-10%; Extended; enable-delay capacitor |
| C2, C9 | 2 | 10 uF, 16 V, X5R | 0805 | C15850 / Samsung CL21A106KAYNNNE | Prototype accepted: user approval 2026-11-11; 10 uF, 25 V, X5R, +/-10%; Basic; verify biased capacitance and rail behavior after assembly |
| C3, C6, C7, C10, C11, C12, C13, C14, C15, C16, C17 | 11 | 100 nF, 16 V | 0603 | C14663 / YAGEO CC0603KRX7R9BB104 | Catalog match: 100 nF, 50 V, X7R, +/-10%; Basic |
| C4, C5, C18 | 3 | 22 uF, 10 V, X5R | 0805 | C602037 / Samsung CL21A226MAYNNNE | Prototype accepted: user approval 2026-11-11; 22 uF, 25 V, X5R, +/-20%; Extended; C18 is the rev B module-local bulk capacitor; verify biased capacitance and regulator response after assembly |
| C8 | 1 | 1 nF, 50 V | 0603 | C106246 / YAGEO CC0603JRNPO9BN102 | Catalog match: 1 nF, 50 V, NP0, +/-5%; Extended; switch slew-rate capacitor |
| F1 | 1 | 0.75 A hold, at least 6 V | 1206 | C269118 / Brightking SMD1206B075TF/16 | Engineering reviewed: reasonable prototype candidate on existing lands; normal assembly review and bench voltage-drop/temperature checks remain |
| L1 | 1 | 3.9 uH, Isat >=3 A | Bourns SRN4018 land pattern | C96899 / Sunlord SWPA4030S3R9MT | Engineering reviewed: terminal envelope fits existing lands for prototype review; 3 mm height approved; supplier electrode-axis/rotation and assembly preview required |
| R1, R2, R3 | 3 | 10 kOhm | 0603 | C25804 / UNI-ROYAL 0603WAF1002T5E | Catalog match: 1%, 100 mW, 75 V; Basic |
| R4, R5 | 2 | 5.1 kOhm, 1% | 0603 | C23186 / UNI-ROYAL 0603WAF5101T5E | Catalog match: 1%, 100 mW, 75 V; Basic |
| R6, R7 | 2 | 22 Ohm | 0603 | C23345 / UNI-ROYAL 0603WAF220JT5E | Catalog match: 1%, 100 mW, 75 V; Basic |
| R8 | 1 | 330 Ohm | 0603 | C23138 / UNI-ROYAL 0603WAF3300T5E | Catalog match: 1%, 100 mW, 75 V; Basic |
| R9, R10 | 2 | 100 kOhm | 0603 | C25803 / UNI-ROYAL 0603WAF1003T5E | Catalog match: 1%, 100 mW, 75 V; Basic |
| R11 | 1 | 100 Ohm | 0603 | C22775 / UNI-ROYAL 0603WAF1000T5E | Catalog match: 1%, 100 mW, 75 V; Basic |

Candidate evidence: JLCPCB part-detail lookups list the values, ratings and
packages above, datasheets and EasyEDA footprints. All eleven resistor positions
use 0603, 1%, 100 mW, 75 V parts. C14663's 50 V rating
exceeds the schematic's 16 V requirement; its listed capacitance is nominal,
not an effective-capacitance measurement. All listed candidates were in stock
at lookup; stock is not reserved. Sources:

- https://www.lcsc.com/product-detail/C14663.html
- https://www.lcsc.com/product-detail/C25804.html
- https://www.lcsc.com/product-detail/C23186.html
- https://www.lcsc.com/product-detail/C23345.html
- https://www.lcsc.com/product-detail/C23138.html
- https://www.lcsc.com/product-detail/C25803.html
- https://www.lcsc.com/product-detail/C22775.html
- https://www.lcsc.com/product-detail/C29936.html
- https://www.lcsc.com/product-detail/C106246.html
- https://www.lcsc.com/product-detail/C15850.html
- https://www.lcsc.com/product-detail/C602037.html
- https://www.lcsc.com/product-detail/C269118.html
- https://www.lcsc.com/product-detail/C96899.html

**Fuse warning:** Previously noted C207016 (Littelfuse 0603L075SLYR) is a
0603, 1.6 x 0.8 mm part, not a match for the existing F1 1206 footprint.
Do not use it as a drop-in replacement.

### Qualification Results And Stop Reasons

The following records distinguish prototype acceptance from full qualification.
On 2026-11-11 the user accepted C2/C9/C4/C5 provisionally for the first assembled
boards, with real-board measurements to follow. Missing capacitor curves are
therefore no longer a first-prototype ordering stop. Datasheets remain the
design baseline; prototype measurements do not establish production guarantees.

- **C2/C9 (two positions):** On 2026-11-11 the user supplied the exact-part
  CL21A106KAYNNNE characterization sheet. The sheet is not included in this
  repository. Its one-page document identity and DC-bias section
  were verified locally; SHA-256:
  `8A03285C8B7134980B9D12C67FE0AC6567633109C859DD37628C462B41E4F80F`.
  Its DC-bias curve indicates roughly
  -50% capacitance at 5 V, or about 5 uF typical per capacitor, measured at 1 kHz,
  1 Vrms. This is a visual graph estimate, not a guaranteed minimum or a
  measurement at the regulator switching conditions. The sheet explicitly
  identifies its curves as typical. Nominal +/-10% tolerance, temperature,
  aging and the AC test-amplitude dependence still matter; do not mark the
  input capacitance adequate from the 10 uF label alone. C2 is regulator input
  decoupling and C9 is on the switched LED rail, so they are not parallel input
  capacitors. The missing-document stop is resolved for these positions, but
  circuit-level effective-capacitance and transient verification is deferred
  to first-board bring-up by the user's prototype acceptance.
- **C4/C5 (two positions):** The user's Samsung lifecycle screenshot identifies
  CL21A226MAQNNN# as NRND and recommends CL21A226MAYNNN#. Replace the procurement
  candidate C45783 / CL21A226MAQNNNE with C602037 / CL21A226MAYNNNE. JLC's catalog
  matches the replacement's 22 uF, 25 V, X5R, +/-20%, 0805 specification; it is
  Extended rather than the old Basic listing. The recommendation is not proof
  of identical bias behavior or guaranteed effective capacitance. A live JLC
  part-detail lookup confirmed 447,818 listed in stock on 2026-11-11; stock is
  not reserved. The replacement's 36-page specification was reviewed but is not
  included in this repository.
  Its first page confirms a 2.00 +/-0.20 x 1.25 +/-0.20 mm body and thickness
  1.25 +/-0.20 mm. Page 14's bias graph is a generic example for a 10 uF,
  6.3 V part, not valid characterization of this 22 uF, 25 V replacement.
  The replacement's own characterization sheet at 3.3 V is desirable but is
  not a first-prototype prerequisite after the user's acceptance. Earlier Samsung
  downloads served an 84-page general catalog; exact-part pages returned HTTP
  500 both through the web reader and a direct request. Stocked
  Murata alternatives were also checked: C440198 / GRM21BR61H106KE43L for 10 uF,
  and C86816 / GRM21BR61E226ME44L for 22 uF. Their supplied PDFs contain generic
  DC-bias warnings but no exact-part curves either. Need actual characteristic
  data from Samsung, Murata SimSurfing or the supplier to establish effective
  capacitance including tolerance at the rail voltage. A higher voltage rating
  alone is not proof. AP63203 datasheet DS41326 Rev. 3-2, pages 13-14, recommends
  3.9 uH, 10 uF input and 2 x 22 uF output, and gives the output-capacitance
  transient calculation; nominal table values do not remove the DC-bias check.
- **F1 (one position):** Brightking SMD1206 series, revision 14-Jun-17, pages
  1-3 and 6-7, confirms C269118's 750 mA hold, 16 V, 1.5 A trip, and 0.35 Ohm
  maximum resistance one hour after reflow/tripping at 23 C. That is up to
  175 mV drop at 500 mA under that stated resistance condition, not a guaranteed
  hot-device voltage-drop bound. Hold current derates to 0.54 A at 60 C and
  0.49 A at 70 C. Its 200 ms trip specification is at 8 A, not at 1.5 A; it is
  not a precise USB current limiter. Actual board pads are 1.25 x 1.75 mm at
  +/-1.4 mm, with a 1.55 mm gap; manufacturer recommended pads are 1.0 x 1.8 mm
  with a 1.8 mm gap. A dimensional review on 2026-11-11 compared these actual
  PCB lands with the datasheet's 3.00-3.40 mm body length, 1.50-1.80 mm width
  and terminal dimensions. With centered placement, the pad outer edge is
  2.025 mm from the part center, leaving at least 0.325 mm beyond the longest
  body. Maximum body/terminal-width overhang is 0.025 mm per side; the longest
  terminal at minimum body length can extend 0.025 mm past the pad's inner
  edge. There is substantial terminal overlap, not an incompatible package.
  This supports retaining C269118 on the existing lands as a reasonable
  prototype candidate, subject to normal assembly review; no special user
  document or footprint change is currently required. It does not certify
  placement tolerance, reflow yield or JLCPCB process acceptance. At 500 mA,
  the stated 0.35 Ohm resistance implies 87.5 mW dissipation as well as the
  175 mV drop; temperature rise cannot be calculated from this alone. Measure
  total current, voltage across F1, downstream rail margin and local temperature
  during first-board tests. Do not claim 500 mA hold across the full temperature
  range: the 70 C derated hold rating is below 500 mA. Escalate only if JLCPCB
  flags assembly or measured thermal/voltage margins are inadequate. No
  footprint change was made; this is prototype engineering review, not full
  thermal or safety qualification.
- **L1 (one position):** No stock result was found for SRN4018-3R9, including
  a live search with zero-stock parts enabled. C96899 is a stocked substitute
  candidate. Sunlord SWPA catalog pages 2, 8 and 16 confirm 3.9 uH +/-20%,
  74 mOhm maximum DCR, listed saturation rating 3 A at approximately 30%
  inductance loss, and listed heating rating 2.1 A at 40 C rise from 20 C.
  These are not interchangeable current definitions. Its body is 4.0 +/-0.2 mm
  square and 3.0 mm maximum high, versus the existing 1.8 mm SRN4018 model.
  Actual L1 pads are 1.5 x 3.6 mm at +/-1.525 mm, gap 1.55 mm; Sunlord recommends
  1.1 x 3.7 mm pads, gap 1.9 mm. A final dimensional review finds the existing
  pads cover the specified 3.3 +/-0.2 mm terminal width with at least 0.05 mm
  per side, and extend 0.175 mm beyond a 4.2 mm maximum body length along the
  terminal axis. This supports a prototype candidate on the existing lands,
  not a guaranteed process-equivalent land pattern. Have JLCPCB verify the
  substituted part's electrode axis, library rotation and soldering preview;
  the CAD still names the original Bourns footprint. The user explicitly accepted the
  3 mm height on 2026-11-11: the housing will be designed after assembled-board
  verification and can provide clearance. Height is no longer a stop; any
  eventual assembly model must represent the actual part. No change to the
  protected PCB was made.

For capacitor bring-up, check regulator input/output and switched LED rail
startup, ripple, load-step response and brownouts at intended operating loads.
Measure capacitance under representative DC bias using suitable equipment and
an isolated part or test coupon; C4/C5 and other parallel circuitry prevent a
simple in-circuit LCR reading from identifying each capacitor's true value.
An unbiased handheld-meter reading alone does not close this validation.

Manufacturer PDFs and rendered dimension pages are not included. No custom-board
current, waveform, thermal or functional hardware tests were run: no assembled
custom PCB exists yet.

This closes the catalog search, not qualification or fabrication approval.
The prototype-accepted capacitors (C2, C9, C4, C5, C18) and engineering-reviewed
F1/L1 may be included in the first-prototype BOM with their acceptance/review
and pending test status retained. Normal assembly preview/DFM approval
still applies to every part.
Do not silently substitute a
different package, increase nominal capacitance, or change the protected layout
to bypass these stops.

## Engineering Gates Still Open

- Freeze every populated manufacturer's part number and JLCPCB/private-library
  match, including inductors, MLCC effective capacitance and resistor tolerances.
- During bring-up, measure the LED data waveform and validate current draw on
  the purchased WS2812B-B-V6 (C52941391); the custom firmware pulse widths match its
  timing table, but no physical waveform has been measured.
- Rev B moved the bootstrap and input capacitors to the regulator pins and
  removed the switch-node vias. Measure ripple, overshoot, startup, load steps,
  input inrush, 3.3 V margin, temperature and EMI on the first boards.
- USB is routed via-free on the rear with flow-through ESD protection. It is
  deliberately not a controlled 90 ohm pair (0.2 mm tracks, 0.8 mm two-layer
  board, front copper under the lines crossed by LED supply tracks); accepted
  by the user for a 10 mm full-speed link. Test both cable orientations.
- The antenna rule area removes copper only; the round outline keeps FR4 under
  the module antenna. Accepted by the user; measure range/throughput if radio
  use is ever enabled.
- Confirm the measured 0.30 mm minimum via drill and annulus with the supplier.
  Rev B has no via centre inside an SMD land. Two ground vias still overlap a
  land with their annular ring only (C8 pad 2 by 0.225 mm, C16 pad 2 by
  0.09 mm); ask the supplier whether tenting is sufficient there. Do not assume
  the retired compact board's fine-drill requirements apply to this 40 mm design.
- Produce matched BOM/CPL, component-rotation review, panelization instructions,
  final Gerbers/drills and firmware artifacts from the same frozen revision.
- Inspect actual JLCPCB assembly previews before approving fabrication and
  placement.
- Rev B native checks report ERC 0, DRC 0, unconnected 0 and schematic parity 0.
  The independent audit confirms all 173 functional schematic pin/net
  assignments and validates the connector geometry. Five native checks remain
  disabled in the project (missing courtyards, track ends not centred on vias,
  tuning-profile geometry, footprint-filter and footprint-type mismatches).
- The pre-existing 40 mm schematic library symbol's *default* footprint filter
  names an ESP32-S2-MINI although the U1 instance and actual PCB both use the
  checked ESP32-S3-MINI footprint. This is a library-default metadata typo, not
  a footprint assignment or hardware defect; it is not a reason to alter the
  protected 40 mm design.

## User Decisions

- **Rev B scope (2026-11-11):** targeted local rework of the USB corridor, the
  buck area and the U2 via-in-pad, plus schematic metadata cleanup. USB gets the
  topology fix only, with 90 ohm explicitly not targeted. Add C18 (22 uF 0805,
  C602037) beside U1 pin 3. Keep the round outline and accept the antenna
  keepout as it is.
- **LED current budget confirmed:** Keep the existing 250 mA LED estimate limit
  for initial prototypes. Revisit only after live current and brightness tests.
- **J1 retained:** Keep the existing 40 mm board's HRO TYPE-C-31-M-12 (C165948)
  and its PCB footprint, based on the "SMT Assembly" listing. Wait for JLCPCB to
  flag any assembly limitation during order review; separate advance confirmation
  is not a release prerequisite. Seek user approval for any resulting change.
- **Bridge support deferred:** Add custom-board support only after assembled
  boards pass basic tests. Use the matching firmware build and physical
  BOOT/RESET for initial programming; do not expand bridge support before then.

## Sources

- https://jlcpcb.com/capabilities/pcb-assembly-capabilities
- https://jlcpcb.com/capabilities/pcb-capabilities
- https://jlcpcb.com/help/article/pcb-assembly-faqs
- https://jlcpcb.com/partdetail/EspressifSystems-ESP32_S3_MINI_1_N8/C2913206
- https://jlcpcb.com/partdetail/HRO-TYPE-C-31-M-12/C165948
- https://jlcpcb.com/partdetail/OmniVision-AP63203WU-7/C780769
- https://jlcpcb.com/partdetail/TexasInstruments-SN74AHCT1G125DBVR/C7484
- https://jlcpcb.com/partdetail/OmniVision-B3U-1000P/C231329
- https://jlcpcb.com/partdetail/TexasInstruments-TPS22918DBVR/C131941
- https://jlcpcb.com/partdetail/STMicroelectronics-USBLC6_2SC6/C7519
- https://jlcpcb.com/partdetail/Littelfuse-0603L075SLYR/C207016
- https://jlcpcb.com/partdetail/Worldsemi-WS2812BB_W/C114586
- https://www.lcsc.com/product-detail/C52941391.html
- https://documentation.espressif.com/esp32-s3-mini-1_mini-1u_datasheet_en.pdf