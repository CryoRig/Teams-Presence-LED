import argparse
import copy
import re
import uuid
import xml.etree.ElementTree as ET
from pathlib import Path

import sexpdata

import check_circuit as source


ROOT = source.ROOT
SCHEMATIC = source.SCHEMATIC
BOARD = ROOT / "teams-presence-led.kicad_pcb"
USB_NETS = {"USB_DP", "USB_DM", "USB_DP_CONN", "USB_DM_CONN"}
# D1 channel swap: pads 1/6 (x=100.95) carry D-, pads 3/4 (x=99.05) carry D+.
D1_LABELS = {
    (57.15, 116.84): ("USB_DP_CONN", "USB_DM_CONN"),
    (67.31, 116.84): ("USB_DP_CONN", "USB_DM_CONN"),
    (57.15, 119.38): ("USB_DM_CONN", "USB_DP_CONN"),
    (67.31, 119.38): ("USB_DM_CONN", "USB_DP_CONN"),
}
C18_ORIGIN = (210.82, 38.1)


def purchasing_overlay():
    choices = {}
    for row in (ROOT / "ORDERING.md").read_text(encoding="utf-8").splitlines():
        if not re.match(r"\| (?:C\d|R\d|F1|L1)", row):
            continue
        columns = [column.strip() for column in row.split("|")]
        references = [reference.strip() for reference in columns[1].split(",")]
        identity = re.fullmatch(r"(C\d+) / (.+) (\S+)", columns[5])
        if identity is None or len(references) != int(columns[2]):
            raise ValueError(f"Invalid purchasing row: {row}")
        for reference in references:
            if reference in choices:
                raise ValueError(f"Duplicate purchasing choice: {reference}")
            choices[reference] = (identity[3], identity[1])
    if len(choices) != 31:
        raise ValueError(f"Expected 31 purchasing-overlay positions, found {len(choices)}")
    return choices


def position(node):
    location = source.child(node, "at")
    return round(float(location[1]), 4), round(float(location[2]), 4)


def update_schematic():
    document = sexpdata.loads(SCHEMATIC.read_text(encoding="utf-8"))
    components = {str(source.properties(symbol)["Reference"][2]): symbol
                  for symbol in source.children(document, "symbol")}
    if "C18" in components or source.children(document, "global_label"):
        raise ValueError("Rev B schematic changes are already applied")

    swapped = 0
    for label in source.children(document, "label"):
        change = D1_LABELS.get(position(label))
        if change is None:
            continue
        if label[1] != change[0]:
            raise ValueError(f"Unexpected D1 label {label[1]} at {position(label)}")
        label[1] = change[1]
        swapped += 1
    if swapped != 4:
        raise ValueError(f"Expected four D1 data labels, found {swapped}")

    template = components["C4"]
    offset_x = C18_ORIGIN[0] - position(template)[0]
    offset_y = C18_ORIGIN[1] - position(template)[1]
    for item in document:
        if source.tagged(item, "lib_symbols"):
            continue
        for location in source.tree_nodes(item, "at"):
            if abs(float(location[1]) - C18_ORIGIN[0]) < 9 and abs(float(location[2]) - C18_ORIGIN[1]) < 9:
                raise ValueError(f"C18 schematic position is occupied: {location}")
    capacitor = copy.deepcopy(template)
    for location in source.tree_nodes(capacitor, "at"):
        location[1] = round(float(location[1]) + offset_x, 4)
        location[2] = round(float(location[2]) + offset_y, 4)
    source.child(capacitor, "uuid")[1] = str(uuid.uuid4())
    for pin in source.children(capacitor, "pin"):
        source.child(pin, "uuid")[1] = str(uuid.uuid4())
    source.properties(capacitor)["Reference"][2] = "C18"
    for reference in source.tree_nodes(capacitor, "reference"):
        reference[1] = "C18"
    document.insert(document.index(components["TP1"]), capacitor)
    components["C18"] = capacitor
    for net, label_y, justify in [("+3V3", 34.29, "left"), ("GND", 41.91, "right")]:
        document.append(source.form(
            f'(label "{net}" (at {C18_ORIGIN[0]} {label_y} 90) '
            f'(effects (font (size 0.9 0.9)) (justify {justify} bottom)) (uuid {uuid.uuid4()}))'))

    overlay = purchasing_overlay()
    for reference, (mpn, lcsc) in overlay.items():
        existing = source.properties(components[reference])
        if "LCSC" in existing or "MPN" in existing:
            raise ValueError(f"{reference} already has a purchasing identity")
        source.set_property(components[reference], "MPN", mpn)
        source.set_property(components[reference], "LCSC", lcsc)
    unsourced = [reference for reference, symbol in components.items()
                 if not reference.startswith(("#", "TP")) and "LCSC" not in source.properties(symbol)]
    if unsourced:
        raise ValueError(f"Components without LCSC identity: {unsourced}")

    for reference in ["TP1", "TP2", "TP3", "TP4"]:
        for flag in ["in_bom", "in_pos_files"]:
            source.child(components[reference], flag)[1] = sexpdata.Symbol("no")

    labels = source.children(document, "label")
    for label in labels:
        label[0] = sexpdata.Symbol("global_label")
        label.append(source.form("(shape bidirectional)"))
    source.child(document, "title_block")[:] = source.form(
        '(title_block (title "Teams Presence Lamp - Round S3") (date "2026-11-11") (rev "B-prototype"))')
    source.write_tree(SCHEMATIC, document)
    print(f"Rev B schematic: D1 channels swapped, C18 added, {len(overlay)} purchasing identities, "
          f"TP1-TP4 excluded from BOM/placement, {len(labels)} global labels")


def update_board(netlist):
    import pcbnew

    board = pcbnew.LoadBoard(str(BOARD))
    footprints = {footprint.GetReference(): footprint for footprint in board.GetFootprints()}
    if len(footprints) != 51 or footprints["C18"].GetLayer() != pcbnew.F_Cu:
        raise ValueError("Expected the synchronized, unreworked board with C18 still unplaced")
    pad_nets = {pad.GetNumber(): pad.GetNetname() for pad in footprints["D1"].Pads()}
    if pad_nets["1"] != "USB_DM_CONN" or pad_nets["3"] != "USB_DP_CONN":
        raise ValueError("D1 channel swap has not been imported from the schematic")

    def point(x, y):
        return pcbnew.VECTOR2I(pcbnew.FromMM(x), pcbnew.FromMM(y))

    def near(position, x, y):
        return abs(pcbnew.ToMM(position.x) - x) < 0.01 and abs(pcbnew.ToMM(position.y) - y) < 0.01

    def is_via(item):
        return item.Type() == pcbnew.PCB_VIA_T

    def rear_segment_in(item, net, left, top, right, bottom):
        if is_via(item) or item.GetNetname() != net or item.GetLayer() != pcbnew.B_Cu:
            return False
        return all(left <= pcbnew.ToMM(end.x) <= right and top <= pcbnew.ToMM(end.y) <= bottom
                   for end in [item.GetStart(), item.GetEnd()])

    def remove(expected, description, predicate):
        items = [item for item in board.GetTracks() if predicate(item)]
        if len(items) != expected:
            raise ValueError(f"{description}: expected {expected} copper items, found {len(items)}")
        for item in items:
            board.Remove(item)

    def route(net, width, *points, layer=pcbnew.B_Cu):
        for start, end in zip(points, points[1:]):
            track = pcbnew.PCB_TRACK(board)
            track.SetStart(point(*start))
            track.SetEnd(point(*end))
            track.SetWidth(pcbnew.FromMM(width))
            track.SetLayer(layer)
            track.SetNet(board.FindNet(net))
            board.Add(track)

    def via(net, x, y, size, drill):
        item = pcbnew.PCB_VIA(board)
        item.SetPosition(point(x, y))
        item.SetWidth(pcbnew.FromMM(size))
        item.SetDrill(pcbnew.FromMM(drill))
        item.SetViaType(pcbnew.VIATYPE_THROUGH)
        item.SetLayerPair(pcbnew.F_Cu, pcbnew.B_Cu)
        item.SetNet(board.FindNet(net))
        board.Add(item)

    def place(reference, x, y, pads):
        footprint = footprints[reference]
        footprint.SetPosition(point(x, y))
        for pad in footprint.Pads():
            if not near(pad.GetPosition(), *pads[pad.GetNumber()]):
                raise ValueError(f"Unexpected {reference} pad {pad.GetNumber()} position")

    schematic = sexpdata.loads(SCHEMATIC.read_text(encoding="utf-8"))
    for symbol in source.children(schematic, "symbol"):
        fields = source.properties(symbol)
        reference = str(fields["Reference"][2])
        if reference.startswith("#"):
            continue
        footprint = footprints[reference]
        for name, field in fields.items():
            if name in {"Reference", "Value", "Footprint"}:
                continue
            created = not footprint.HasField(name)
            footprint.SetField(name, str(field[2]))
            if created:
                added = footprint.GetField(name)
                added.SetVisible(False)
                added.SetLayer(pcbnew.B_Fab if footprint.GetLayer() == pcbnew.B_Cu else pcbnew.F_Fab)
    unused = 0
    for net in ET.parse(netlist).findall("./nets/net"):
        name = net.attrib["name"]
        if not name.startswith("unconnected-"):
            continue
        unused_net = board.FindNet(name)
        if unused_net is None:
            unused_net = pcbnew.NETINFO_ITEM(board, name)
            board.Add(unused_net)
        for node in net.findall("node"):
            for pad in footprints[node.attrib["ref"]].Pads():
                if pad.GetNumber() == node.attrib["pin"]:
                    pad.SetNet(unused_net)
                    unused += 1

    # USB: both lines stay on the rear, straight through D1 and the series resistors.
    remove(38, "USB", lambda item: item.GetNetname() in USB_NETS)
    remove(4, "D1 VBUS feed", lambda item: rear_segment_in(item, "VBUS", 97.3, 106.3, 100.05, 107.9)
           or (is_via(item) and near(item.GetPosition(), 97.3353, 106.3305)))
    remove(1, "D1 ground via", lambda item: is_via(item) and near(item.GetPosition(), 99.75, 109.25))
    place("R6", 99.15, 104.4, {"1": (99.15, 105.225), "2": (99.15, 103.575)})
    place("R7", 100.85, 104.4, {"1": (100.85, 105.225), "2": (100.85, 103.575)})
    route("USB_DP_CONN", 0.2, (99.25, 112.855), (99.25, 112.1214), (99.5337, 111.8377),
          (99.9206, 111.8377), (100.25, 112.1671), (100.25, 112.855))
    route("USB_DP_CONN", 0.2, (99.5337, 111.8377), (99.05, 111.354), (99.05, 110.1375),
          (99.05, 107.8625), (99.05, 105.925), (99.15, 105.825), (99.15, 105.225))
    route("USB_DM_CONN", 0.2, (99.75, 112.855), (99.75, 113.5748), (100.0196, 113.8444),
          (100.5048, 113.8444), (100.75, 113.5992), (100.75, 112.855), (100.75, 111.4),
          (100.95, 111.2), (100.95, 110.1375), (100.95, 107.8625), (100.95, 105.325), (100.85, 105.225))
    route("USB_DP", 0.2, (99.15, 103.575), (99.15, 101.83))
    route("USB_DM", 0.2, (100.85, 103.575), (100.85, 103.3), (100, 102.45), (100, 101.83))
    via("VBUS", 100, 106.6, 0.7, 0.35)
    route("VBUS", 0.4, (100, 106.6), (100, 107.8625))
    route("VBUS", 0.375, (100, 106.6), (99.9414, 107.1502), layer=pcbnew.F_Cu)
    via("GND", 99.7, 109.15, 0.6, 0.3)
    route("GND", 0.25, (99.7, 109.15), (100, 110.1375))

    # Buck: bootstrap capacitor at pins 5/6, input capacitor at pins 3/4, no via in the pin 4 land.
    remove(16, "BUCK_SW", lambda item: item.GetNetname() == "BUCK_SW")
    remove(7, "BUCK_BST", lambda item: item.GetNetname() == "BUCK_BST")
    remove(13, "3V3 feedback", lambda item: rear_segment_in(item, "+3V3", 84.8, 103.9, 90.7, 110.1)
           and max(item.GetStart().y, item.GetEnd().y) > pcbnew.FromMM(104.01))
    remove(2, "C2 5V link", lambda item: rear_segment_in(item, "+5V", 90.1, 110.3, 90.7, 111.65))
    remove(1, "U2 ground via", lambda item: is_via(item) and near(item.GetPosition(), 88, 109))
    place("C3", 90.5, 105.5, {"1": (91.275, 105.5), "2": (89.725, 105.5)})
    place("C2", 89.5, 110.8, {"1": (90.45, 110.8), "2": (88.55, 110.8)})
    route("BUCK_SW", 0.5, (87.925, 104), (87.925, 106.252), (87.6421, 106.252),
          (87.2298, 106.664), (87.2298, 107.801))
    route("BUCK_SW", 0.375, (87.2298, 107.801), (88.1633, 107.801), (88.3625, 108))
    route("BUCK_SW", 0.5, (87.925, 105.5), (89.725, 105.5))
    route("BUCK_BST", 0.2, (88.3625, 107.05), (89.0525, 106.36), (91, 106.36),
          (91.275, 106.085), (91.275, 105.5))
    route("+3V3", 0.3, (90.6375, 107.05), (92, 107.05), (92.3, 106.75), (92.3, 104.3),
          (92, 104), (90.5, 104), (90.2, 103.7), (90.2, 101.15), (88.45, 99.4))
    route("+5V", 0.5, (90.6375, 110.305), (90.45, 110.8))
    route("GND", 0.5, (88.3625, 108.95), (88.3625, 110.3), (88.55, 110.8))
    route("GND", 0.4, (88.55, 110.8), (87.7, 110.8), (87.2, 110.3))
    route("GND", 0.4, (87.7, 110.8), (87.2, 111.3))
    via("GND", 87.2, 110.3, 0.6, 0.3)
    via("GND", 87.2, 111.3, 0.6, 0.3)

    # Module supply: 22 uF beside the 100 nF at U1 pin 3, ground returned to U1 pins 1/2.
    remove(4, "C6 3V3 link", lambda item: rear_segment_in(item, "+3V3", 107.9, 90.1, 110.9, 92.6))
    # C6 stays clear of U1's pin 1 silkscreen dot at (108.2, 88.5).
    place("C6", 109.05, 89.4, {"1": (109.05, 90.175), "2": (109.05, 88.625)})
    capacitor = footprints["C18"]
    capacitor.SetOrientationDegrees(90)
    capacitor.Flip(capacitor.GetPosition(), pcbnew.FLIP_DIRECTION_LEFT_RIGHT)
    capacitor.Reference().SetVisible(False)
    capacitor.Value().SetVisible(False)
    place("C18", 110.85, 89.4, {"1": (110.85, 90.35), "2": (110.85, 88.45)})
    route("+3V3", 0.4, (107.981, 90.3502), (110.825, 90.3502))
    route("+3V3", 0.4, (109.05, 90.175), (109.05, 90.3502))
    route("+3V3", 0.5, (110.825, 90.3502), (110.825, 92.5))
    route("GND", 0.3, (110.85, 88.45), (109.225, 88.45), (109.05, 88.625), (107.605, 88.625),
          (107.38, 88.85), (106.98, 88.85))
    # C18 closes the rear ground channel that fed C9; give C9 its own via to the front plane.
    via("GND", 114.2, 95.1, 0.6, 0.3)
    route("GND", 0.3, (113.35, 93.6), (113.35, 94.25), (114.2, 95.1))

    marked = 0
    for drawing in board.GetDrawings():
        if drawing.Type() == pcbnew.PCB_TEXT_T and drawing.GetText() == "S3  REV A":
            drawing.SetText("S3  REV B")
            marked += 1
    if marked != 1:
        raise ValueError("Revision marking not found")
    board.GetTitleBlock().SetRevision("B-prototype")
    board.GetTitleBlock().SetDate("2026-11-11")
    pcbnew.ZONE_FILLER(board).Fill(board.Zones())
    pcbnew.SaveBoard(str(BOARD), board)
    print(f"Rev B board: fields synchronized, {unused} unused pins named, USB/buck/module-supply rework applied")


def verify_board(reference):
    import heapq
    import math

    import pcbnew

    board = pcbnew.LoadBoard(str(BOARD))
    footprints = {footprint.GetReference(): footprint for footprint in board.GetFootprints()}
    tracks = list(board.GetTracks())
    vias = [item for item in tracks if item.Type() == pcbnew.PCB_VIA_T]
    segments = [item for item in tracks if item.Type() != pcbnew.PCB_VIA_T]

    def pad(reference, number):
        return next(item for item in footprints[reference].Pads() if item.GetNumber() == number)

    def key(position):
        return round(pcbnew.ToMM(position.x), 3), round(pcbnew.ToMM(position.y), 3)

    def path(net, start, end):
        graph = {}
        for item in segments:
            if item.GetNetname() == net:
                first, second = key(item.GetStart()), key(item.GetEnd())
                graph.setdefault(first, []).append((second, math.dist(first, second)))
                graph.setdefault(second, []).append((first, math.dist(first, second)))
        target, queue, seen = key(pad(*end).GetPosition()), [(0, key(pad(*start).GetPosition()))], set()
        while queue:
            length, node = heapq.heappop(queue)
            if node == target:
                return length
            if node not in seen:
                seen.add(node)
                for neighbour, step in graph.get(node, []):
                    heapq.heappush(queue, (length + step, neighbour))
        raise ValueError(f"No {net} copper path from {start} to {end}")

    usb = [item for item in tracks if item.GetNetname() in USB_NETS]
    if any(item.Type() == pcbnew.PCB_VIA_T or item.GetLayer() != pcbnew.B_Cu for item in usb):
        raise ValueError("USB data copper must be via-free on the rear")
    for side, plus, minus in [("A", "A6", "A7"), ("B", "B6", "B7")]:
        positive = path("USB_DP_CONN", ("J1", plus), ("R6", "1")) + path("USB_DP", ("R6", "2"), ("U1", "24"))
        negative = path("USB_DM_CONN", ("J1", minus), ("R7", "1")) + path("USB_DM", ("R7", "2"), ("U1", "23"))
        print(f"USB orientation {side}: D+ {positive:.3f} mm, D- {negative:.3f} mm, "
              f"difference {abs(positive - negative):.3f} mm (copper between pad centres, resistors excluded)")
    for net in ["BUCK_SW", "BUCK_BST"]:
        if any(item.GetNetname() == net for item in vias):
            raise ValueError(f"{net} must not use vias")
        print(f"{net}: {sum(pcbnew.ToMM(item.GetLength()) for item in segments if item.GetNetname() == net):.3f} mm, no vias")
    for item in vias:
        x, y, radius = item.GetPosition().x, item.GetPosition().y, item.GetWidth(pcbnew.F_Cu) // 2
        for footprint in footprints.values():
            for land in footprint.Pads():
                box = land.GetBoundingBox()
                if land.GetAttribute() != pcbnew.PAD_ATTRIB_SMD:
                    continue
                gap = math.hypot(max(box.GetLeft() - x, 0, x - box.GetRight()), max(box.GetTop() - y, 0, y - box.GetBottom()))
                if gap == 0:
                    raise ValueError(f"Via centre inside {footprint.GetReference()} pad {land.GetNumber()}")
                if gap < radius:
                    print(f"Via annulus overlaps land by {pcbnew.ToMM(radius - gap):.3f} mm: {item.GetNetname()} via at {key(item.GetPosition())}, "
                          f"{footprint.GetReference()} pad {land.GetNumber()}")
    print(f"C18 pad 1 to U1 pad 3: {math.dist(key(pad('C18', '1').GetPosition()), key(pad('U1', '3').GetPosition())):.3f} mm centre to centre")
    print(f"PASS: {len(footprints)} footprints; USB via-free on the rear; no via centre in an SMD land")
    if reference is not None:
        previous = pcbnew.LoadBoard(str(reference))
        fixed = ["U1", "J1", "SW1", "SW2", *[f"D{index}" for index in range(2, 10)]]
        for ref in fixed:
            old, new = previous.FindFootprintByReference(ref), footprints[ref]
            if old is None:
                raise ValueError(f"{ref} missing from reference board")
            if (old.GetPosition() != new.GetPosition() or old.GetLayer() != new.GetLayer()
                    or old.GetOrientationDegrees() != new.GetOrientationDegrees()):
                raise ValueError(f"{ref} moved from its reference placement")
        areas = [[zone.GetBoundingBox() for zone in item.Zones() if zone.GetIsRuleArea()] for item in [previous, board]]
        if (previous.GetBoardEdgesBoundingBox() != board.GetBoardEdgesBoundingBox() or len(areas[1]) != 1
                or areas[0] != areas[1]):
            raise ValueError("Outline or antenna keepout differs from reference board")
        print(f"PASS: outline, antenna keepout and {len(fixed)} mechanical placements match reference board")
    else:
        print("NOT CHECKED: outline, antenna keepout and mechanical placements against a reference board")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--schematic", action="store_true",
                        help="Apply the rev B schematic changes once; close the schematic editor first")
    parser.add_argument("--board", type=Path, metavar="NETLIST",
                        help="Apply the rev B board rework once, after updating the PCB from the schematic; "
                             "close the PCB editor first")
    parser.add_argument("--verify", action="store_true", help="Report rev B layout metrics without modifying files")
    parser.add_argument("--reference", type=Path, help="Compare rev B placement and outline against a reference board")
    args = parser.parse_args()
    if args.reference and not args.verify:
        parser.error("--reference requires --verify")
    if args.schematic:
        update_schematic()
    if args.board:
        update_board(args.board)
    if args.verify:
        verify_board(args.reference)
    if not args.schematic and not args.board and not args.verify:
        parser.error("Choose --schematic, --board or --verify")
