import argparse
import copy
import math
from pathlib import Path
import uuid
import xml.etree.ElementTree as ET

import sexpdata


ROOT = Path(__file__).resolve().parents[1]
SCHEMATIC = ROOT / "TPL01B.kicad_sch"
CONNECTIONS = {
    "U1": {"1": "GND", "2": "GND", "3": "+3V3", "4": "BOOT",
           "6": "LED_GPIO", "8": "LED_PWR_EN", "23": "USB_DM", "24": "USB_DP",
           "39": "UART_TX", "40": "UART_RX", "44": "STRAP46", "45": "EN"},
    "R1": {"1": "+3V3", "2": "EN"},
    "R2": {"1": "+3V3", "2": "BOOT"},
    "R3": {"1": "STRAP46", "2": "GND"},
    "C1": {"1": "EN", "2": "GND"},
    "SW1": {"1": "EN", "2": "GND"},
    "SW2": {"1": "BOOT", "2": "GND"},
    "J1": {"A1": "GND", "A12": "GND", "B1": "GND", "B12": "GND", "SH": "GND",
           "A4": "VBUS", "A9": "VBUS", "B4": "VBUS", "B9": "VBUS",
           "A5": "CC1", "B5": "CC2", "A6": "USB_DP_CONN", "B6": "USB_DP_CONN",
           "A7": "USB_DM_CONN", "B7": "USB_DM_CONN"},
    "R4": {"1": "CC1", "2": "GND"},
    "R5": {"1": "CC2", "2": "GND"},
    "R6": {"1": "USB_DP_CONN", "2": "USB_DP"},
    "R7": {"1": "USB_DM_CONN", "2": "USB_DM"},
    "D1": {"1": "USB_DM_CONN", "6": "USB_DM_CONN", "3": "USB_DP_CONN",
           "4": "USB_DP_CONN", "2": "GND", "5": "VBUS"},
    "F1": {"1": "VBUS", "2": "+5V"},
    "U2": {"1": "+3V3", "2": "+5V", "3": "+5V", "4": "GND",
           "5": "BUCK_SW", "6": "BUCK_BST"},
    "L1": {"1": "BUCK_SW", "2": "+3V3"},
    "C2": {"1": "+5V", "2": "GND"},
    "C3": {"1": "BUCK_BST", "2": "BUCK_SW"},
    "C4": {"1": "+3V3", "2": "GND"},
    "C5": {"1": "+3V3", "2": "GND"},
    "C6": {"1": "+3V3", "2": "GND"},
    "C18": {"1": "+3V3", "2": "GND"},
    "U3": {"1": "GND", "2": "LED_GPIO", "3": "GND", "4": "LED_BUF", "5": "LED_5V"},
    "R8": {"1": "LED_BUF", "2": "LED_DIN"},
    "R9": {"1": "LED_GPIO", "2": "GND"},
    "C7": {"1": "LED_5V", "2": "GND"},
    "U4": {"1": "+5V", "2": "GND", "3": "LED_PWR_EN", "4": "LED_CT",
           "5": "LED_QOD", "6": "LED_5V"},
    "C8": {"1": "LED_CT", "2": "GND"},
    "R10": {"1": "LED_PWR_EN", "2": "GND"},
    "R11": {"1": "LED_QOD", "2": "LED_5V"},
    "C9": {"1": "LED_5V", "2": "GND"},
    "TP1": {"1": "+3V3"}, "TP2": {"1": "GND"},
    "TP3": {"1": "UART_TX"}, "TP4": {"1": "UART_RX"},
    "#FLG01": {"1": "VBUS"}, "#FLG02": {"1": "GND"},
    "#FLG03": {"1": "+3V3"}, "#FLG04": {"1": "+5V"},
}
for number in [42, 43, *range(46, 66)]:
    CONNECTIONS["U1"][str(number)] = "GND"
for index in range(8):
    CONNECTIONS[f"D{index + 2}"] = {
        "1": "LED_5V", "3": "GND", "4": "LED_DIN" if index == 0 else f"PIXEL_{index}"
    }
    if index < 7:
        CONNECTIONS[f"D{index + 2}"]["2"] = f"PIXEL_{index + 1}"
    CONNECTIONS[f"C{index + 10}"] = {"1": "LED_5V", "2": "GND"}

SOURCING = {
    "U1": ("ESP32-S3-MINI-1-N8", "C2913206"),
    "J1": ("TYPE-C-31-M-12", "C165948"),
    "U2": ("AP63203WU-7", "C780769"),
    "U3": ("SN74AHCT1G125DBVR", "C7484"),
    "U4": ("TPS22918DBVR", "C131941"),
    "D1": ("USBLC6-2SC6", "C7519"),
    "SW1": ("B3U-1000P", "C231329"),
    "SW2": ("B3U-1000P", "C231329"),
}
for index in range(2, 10):
    SOURCING[f"D{index}"] = ("WS2812B-B-V6", "C52941391")


def tagged(node, name):
    return isinstance(node, list) and bool(node) and str(node[0]) == name


def children(node, name):
    return [child for child in node if tagged(child, name)]


def child(node, name):
    return next(iter(children(node, name)), None)


def tree_nodes(node, name):
    if not isinstance(node, list):
        return
    if tagged(node, name):
        yield node
    for element in node:
        yield from tree_nodes(element, name)


def form(text):
    return sexpdata.loads(text)


def write_tree(path, tree):
    text = "(" + str(tree[0]) + "\n" + "\n".join("  " + sexpdata.dumps(item) for item in tree[1:]) + "\n)\n"
    path.write_text(text, encoding="ascii")


def properties(component):
    return {entry[1]: entry for entry in children(component, "property")}


def set_property(component, name, value):
    props = properties(component)
    if name in props:
        props[name][2] = value
    else:
        component.append(form(f'(property "{name}" "{value}" (at 0 0 0) (effects (font (size 1 1)) (hide yes)))'))


def wire_schematic():
    document = sexpdata.loads(SCHEMATIC.read_text(encoding="utf-8"))
    document[:] = [item for item in document if not any(tagged(item, name) for name in
                  ["wire", "label", "global_label", "junction", "no_connect", "text", "title_block"])]
    document[:] = [item for item in document if not (tagged(item, "symbol") and
                  properties(item)["Reference"][2] == "#FLG05")]
    library = {entry[1]: entry for entry in children(child(document, "lib_symbols"), "symbol")}
    seen_refs = set()
    labels = set()
    for component in children(document, "symbol"):
        reference = properties(component)["Reference"][2]
        if reference in seen_refs:
            raise ValueError(f"Duplicate reference {reference}")
        seen_refs.add(reference)
        if reference not in CONNECTIONS:
            raise ValueError(f"Unexpected component {reference}")
        symbol_id = child(component, "lib_id")[1]
        definition = library[symbol_id]
        origin = child(component, "at")
        angle = math.radians(float(origin[3]))
        seen_pins = set()
        for pin in tree_nodes(definition, "pin"):
            number = str(child(pin, "number")[1])
            if number in seen_pins:
                continue
            seen_pins.add(number)
            location = child(pin, "at")
            local_x, local_y = float(location[1]), float(location[2])
            position_x = round(float(origin[1]) + local_x * math.cos(angle) - local_y * math.sin(angle), 4)
            position_y = round(float(origin[2]) - local_x * math.sin(angle) - local_y * math.cos(angle), 4)
            net = CONNECTIONS[reference].get(number)
            if net is None:
                document.append(form(f'(no_connect (at {position_x} {position_y}) (uuid {uuid.uuid4()}))'))
                continue
            label_key = (position_x, position_y, net)
            if label_key in labels:
                continue
            labels.add(label_key)
            pin_angle = (float(location[3]) + float(origin[3])) % 360
            justify = "right" if pin_angle in (0, 90) else "left"
            label_angle = 90 if pin_angle in (90, 270) else 0
            document.append(form(f'(label "{net}" (at {position_x} {position_y} {label_angle}) '
                            f'(effects (font (size 0.9 0.9)) (justify {justify} bottom)) (uuid {uuid.uuid4()}))'))
        if set(CONNECTIONS[reference]) - seen_pins:
            raise ValueError(f"Unknown pins on {reference}: {set(CONNECTIONS[reference]) - seen_pins}")
        if reference in SOURCING:
            mpn, lcsc = SOURCING[reference]
            set_property(component, "MPN", mpn)
            set_property(component, "LCSC", lcsc)
        if reference == "U1":
            set_property(component, "Footprint", "LampParts:BULETM-SMD_ESP32-S3-MINI-1-N8")
        if reference in {"SW1", "SW2"}:
            set_property(component, "Footprint", "Button_Switch_SMD:SW_SPST_B3U-1000P")
    if seen_refs != set(CONNECTIONS):
        raise ValueError(f"Missing components: {set(CONNECTIONS) - seen_refs}")
    for symbol_id, definition in library.items():
        if symbol_id.startswith("LampParts:"):
            for pin in tree_nodes(definition, "pin"):
                pin[1] = sexpdata.Symbol("passive")
                if symbol_id.endswith("TPS22918DBVR"):
                    number = str(child(pin, "number")[1])
                    pin[1] = sexpdata.Symbol({"1": "power_in", "2": "power_in", "3": "input", "6": "power_out"}.get(number, "passive"))
    document.append(form('(title_block (title "Teams Presence Lamp - Round S3") (date "2026-11-09") (rev "A-prototype"))'))
    for text, position_x, position_y in [
        ("USB-C / ESD / native USB 2.0", 24, 24),
        ("ESP32-S3-MINI-1-N8 / hardware ROM recovery", 110, 24),
        ("Eight WS2812B RGB pixels - clockwise on PCB", 230, 24),
        ("5V USB input / 3.3V buck", 24, 133),
        ("3.3V to 5V LED data", 143, 133),
        ("Slew-controlled LED rail: GPIO4, off during startup", 105, 204),
        ("BOOT: hold BOOT, press/release RESET, release BOOT", 230, 211),
        ("Prototype: power, LED waveform and RF require bench validation", 230, 224),
    ]:
        document.append(form(f'(text "{text}" (at {position_x} {position_y} 0) (effects (font (size 1.2 1.2)) (justify left)) (uuid {uuid.uuid4()}))'))
    write_tree(SCHEMATIC, document)
    print(f"Wired {len(seen_refs)} symbols with {len(labels)} pin labels; all other pins explicitly NC")


def replace_connector():
    document = sexpdata.loads(SCHEMATIC.read_text(encoding="utf-8"))
    library = child(document, "lib_symbols")
    component = next(entry for entry in children(document, "symbol")
                     if properties(entry)["Reference"][2] == "J1")
    old_id = child(component, "lib_id")[1]
    new_id = "Connector:USB_C_Receptacle_USB2.0_16P"
    if old_id == new_id:
        set_property(component, "Footprint", "LampParts:USB_C_Receptacle_HRO_TYPE-C-31-M-12")
        write_tree(SCHEMATIC, document)
        print("Updated J1 to the project-local, model-verified right-angle footprint")
        return
    old_definition = next(entry for entry in children(library, "symbol") if entry[1] == old_id)
    origin = child(component, "at")
    angle = math.radians(float(origin[3]))
    old_locations = set()
    for pin in tree_nodes(old_definition, "pin"):
        location = child(pin, "at")
        old_locations.add((round(float(origin[1]) + float(location[1]) * math.cos(angle)
                                 - float(location[2]) * math.sin(angle), 4),
                           round(float(origin[2]) - float(location[1]) * math.sin(angle)
                                 - float(location[2]) * math.cos(angle), 4)))
    retained = []
    for entry in document:
        location = child(entry, "at") if isinstance(entry, list) else None
        attached = location and (round(float(location[1]), 4), round(float(location[2]), 4)) in old_locations
        if attached and (tagged(entry, "label") or tagged(entry, "no_connect")):
            continue
        retained.append(entry)
    document[:] = retained
    source = sexpdata.loads(Path("C:/Program Files/KiCad/10.0/share/kicad/symbols/Connector.kicad_sym").read_text(encoding="utf-8"))
    definition = copy.deepcopy(next(entry for entry in children(source, "symbol")
                                    if entry[1] == "USB_C_Receptacle_USB2.0_16P"))
    definition[1] = new_id
    library.append(definition)
    child(component, "lib_id")[1] = new_id
    component[:] = [entry for entry in component if not tagged(entry, "pin")]
    set_property(component, "Value", "TYPE-C-31-M-12")
    set_property(component, "Footprint", "LampParts:USB_C_Receptacle_HRO_TYPE-C-31-M-12")
    set_property(component, "Datasheet", "https://jlcpcb.com/api/file/downloadByFileSystemAccessId/8550723676069908480")
    set_property(component, "MPN", "TYPE-C-31-M-12")
    set_property(component, "LCSC", "C165948")
    labels = set()
    for pin in tree_nodes(definition, "pin"):
        number = str(child(pin, "number")[1])
        location = child(pin, "at")
        position_x = round(float(origin[1]) + float(location[1]) * math.cos(angle)
                           - float(location[2]) * math.sin(angle), 4)
        position_y = round(float(origin[2]) - float(location[1]) * math.sin(angle)
                           - float(location[2]) * math.cos(angle), 4)
        net = CONNECTIONS["J1"].get(number)
        if net is None:
            document.append(form(f'(no_connect (at {position_x} {position_y}) (uuid {uuid.uuid4()}))'))
        elif (position_x, position_y, net) not in labels:
            labels.add((position_x, position_y, net))
            pin_angle = (float(location[3]) + float(origin[3])) % 360
            justify = "right" if pin_angle in (0, 90) else "left"
            label_angle = 90 if pin_angle in (90, 270) else 0
            document.append(form(f'(label "{net}" (at {position_x} {position_y} {label_angle}) '
                                 f'(effects (font (size 0.9 0.9)) (justify {justify} bottom)) (uuid {uuid.uuid4()}))'))
    backup = ROOT / "outputs" / "before-right-angle.kicad_sch"
    if not backup.exists():
        backup.write_bytes(SCHEMATIC.read_bytes())
    write_tree(SCHEMATIC, document)
    print("Migrated only J1 to the standard 16-contact right-angle USB-C symbol and matching footprint")


def audit_netlist(path):
    document = ET.parse(path)
    actual = {}
    for net in document.findall("./nets/net"):
        name = net.attrib["name"].removeprefix("/")
        for node in net.findall("node"):
            actual[(node.attrib["ref"], node.attrib["pin"])] = name
    errors = []
    for reference, pins in CONNECTIONS.items():
        if reference.startswith("#"):
            continue
        for pin, expected in pins.items():
            if actual.get((reference, pin)) != expected:
                errors.append(f"{reference}/{pin}: expected {expected}, got {actual.get((reference, pin))}")
    for (reference, pin), name in actual.items():
        if reference in CONNECTIONS and pin not in CONNECTIONS[reference] and not name.startswith("unconnected-"):
            errors.append(f"Unexpected connected pin {reference}/{pin}: {name}")
    if errors:
        raise ValueError("\n".join(errors))
    print(f"PASS: {sum(len(pins) for ref, pins in CONNECTIONS.items() if not ref.startswith('#'))} pin/net assignments match native KiCad netlist")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--wire", action="store_true", help="Rebuild schematic wiring from this reviewed circuit definition; close schematic editor first")
    parser.add_argument("--netlist", type=Path, help="Audit a native KiCad XML netlist without modifying files")
    parser.add_argument("--replace-connector", action="store_true")
    args = parser.parse_args()
    if args.wire:
        wire_schematic()
    if args.replace_connector:
        replace_connector()
    if args.netlist:
        audit_netlist(args.netlist)
    if not args.wire and not args.netlist and not args.replace_connector:
        parser.error("Choose --wire or --netlist")