import argparse
import json
import math
import shutil
from pathlib import Path
import xml.etree.ElementTree as ET

import pcbnew


ROOT = Path(__file__).resolve().parents[1]
BOARD = ROOT / "TPL01B.kicad_pcb"


def repair_module_ground(board):
    name = "BULETM-SMD_ESP32-S3-MINI-1-N8"
    source = pcbnew.FootprintLoad(str(ROOT / "LampParts.pretty"), name)
    placed = board.FindFootprintByReference("U1")
    ground = board.FindNet("GND")
    for number, position_x, position_y in [("62", -7, -7), ("63", -7, 7), ("64", 7, 7), ("65", 7, -7)]:
        source_position = pcbnew.VECTOR2I(pcbnew.FromMM(position_x), pcbnew.FromMM(position_y))
        source_pad = min(source.Pads(), key=lambda pad: (pad.GetPosition() - source_position).SquaredEuclideanNorm())
        if (source_pad.GetPosition() - source_position).SquaredEuclideanNorm() > pcbnew.FromMM(0.1) ** 2:
            raise ValueError(f"Missing module corner {number}")
        source_pad.SetNumber(number)
        placed_position = placed.GetPosition() + pcbnew.VECTOR2I(pcbnew.FromMM(-position_x), pcbnew.FromMM(position_y))
        placed_pad = min(placed.Pads(), key=lambda pad: (pad.GetPosition() - placed_position).SquaredEuclideanNorm())
        if (placed_pad.GetPosition() - placed_position).SquaredEuclideanNorm() > pcbnew.FromMM(0.1) ** 2:
            raise ValueError(f"Unexpected placed module orientation at {number}")
        placed_pad.SetNumber(number)
        placed_pad.SetNet(ground)
    source.SetFPID(pcbnew.LIB_ID("LampParts", name))
    pcbnew.FootprintSave(str(ROOT / "LampParts.pretty"), source)


def add_ground_zones(board):
    ground = board.FindNet("GND")
    if ground is None:
        raise ValueError("Missing GND net")
    for existing in list(board.Zones()):
        if not existing.GetIsRuleArea() and existing.GetNetname() == "GND":
            board.Remove(existing)
    for layer in [pcbnew.F_Cu, pcbnew.B_Cu]:
        zone = pcbnew.ZONE(board)
        zone.SetLayer(layer)
        zone.SetNetCode(ground.GetNetCode())
        zone.SetLocalClearance(pcbnew.FromMM(0.2))
        zone.SetMinThickness(pcbnew.FromMM(0.2))
        zone.SetThermalReliefGap(pcbnew.FromMM(0.2))
        zone.SetThermalReliefSpokeWidth(pcbnew.FromMM(0.25))
        outline = zone.Outline()
        outline.NewOutline()
        for position_x, position_y in [(80.2, 80.2), (119.8, 80.2), (119.8, 119.8), (80.2, 119.8)]:
            outline.Append(pcbnew.FromMM(position_x), pcbnew.FromMM(position_y))
        board.Add(zone)


def audit(board, netlist):
    footprints = {footprint.GetReference(): footprint for footprint in board.GetFootprints()}
    if len(footprints) != 51 or board.GetCopperLayerCount() != 2:
        raise ValueError("Expected 51 footprints on exactly two copper layers")
    box = board.GetBoardEdgesBoundingBox()
    if abs(pcbnew.ToMM(box.GetWidth()) - 40) > 0.1 or abs(pcbnew.ToMM(box.GetHeight()) - 40) > 0.1:
        raise ValueError("Board is not nominally 40 mm")
    angles = []
    for index in range(2, 10):
        led = footprints[f"D{index}"]
        position = led.GetPosition()
        offset_x, offset_y = pcbnew.ToMM(position.x) - 100, pcbnew.ToMM(position.y) - 100
        if led.GetLayer() != pcbnew.F_Cu or abs(math.hypot(offset_x, offset_y) - 10.5) > 0.001:
            raise ValueError(f"LED {index} is not on the front 10.5 mm radius")
        angles.append(math.degrees(math.atan2(offset_y, offset_x)) % 360)
    angles.sort()
    if any(abs((angles[(index + 1) % 8] - angles[index]) % 360 - 45) > 0.01 for index in range(8)):
        raise ValueError("LEDs are not equally spaced")
    if any(footprints[reference].GetLayer() != pcbnew.B_Cu for reference in ["U1", "J1", "SW1", "SW2"]):
        raise ValueError("Controller, USB and recovery switches must be on the rear")
    assigned = 0
    for net in ET.parse(netlist).findall("./nets/net"):
        expected = net.attrib["name"].removeprefix("/")
        if expected.startswith("unconnected-"):
            continue
        for node in net.findall("node"):
            reference, number = node.attrib["ref"], node.attrib["pin"]
            pads = [pad for pad in footprints[reference].Pads() if pad.GetNumber() == number]
            if not pads or any(pad.GetNetname() != expected for pad in pads):
                raise ValueError(f"PCB net mismatch: {reference}/{number} expected {expected}")
            assigned += 1
    for reference, footprint in footprints.items():
        for pad in footprint.Pads():
            if pad.GetAttribute() == pcbnew.PAD_ATTRIB_PTH and reference != "J1":
                raise ValueError(f"Unapproved through-hole component {reference}")
    print(f"PASS: two layers, 40 mm board, eight equally spaced front LEDs, rear USB/recovery, {assigned} native netlist assignments")


def report_routing(board):
    tracks = list(board.GetTracks())
    report = {}
    for name in ["USB_DP_CONN", "USB_DM_CONN", "USB_DP", "USB_DM", "VBUS", "+5V", "+3V3", "LED_5V", "BUCK_SW"]:
        items = [item for item in tracks if item.GetNetname() == name]
        segments = [item for item in items if not isinstance(item, pcbnew.PCB_VIA)]
        report[name] = {
            "length_mm": round(sum(pcbnew.ToMM(item.GetLength()) for item in segments), 3),
            "min_width_mm": round(min((pcbnew.ToMM(item.GetWidth()) for item in segments), default=0), 3),
            "vias": sum(isinstance(item, pcbnew.PCB_VIA) for item in items),
            "layers": sorted({board.GetLayerName(item.GetLayer()) for item in segments}),
        }
    print(json.dumps(report, indent=2))


def report_pads(board):
    selected = {
        "J1": {"A6", "A7", "B6", "B7", "A1", "B1"},
        "U1": {"23", "24", "3", "45"},
        "D1": {"1", "2", "3", "4", "5", "6"},
        "R6": {"1", "2"}, "R7": {"1", "2"},
        "U2": {"1", "2", "3", "4", "5", "6"},
        "L1": {"1", "2"}, "C2": {"1", "2"}, "C3": {"1", "2"},
    }
    for footprint in board.GetFootprints():
        reference = footprint.GetReference()
        for pad in footprint.Pads():
            if pad.GetNumber() in selected.get(reference, set()):
                position, size = pad.GetPosition(), pad.GetSize()
                print(reference, pad.GetNumber(), pad.GetNetname(),
                      round(pcbnew.ToMM(position.x), 4), round(pcbnew.ToMM(position.y), 4),
                      round(pcbnew.ToMM(size.x), 4), round(pcbnew.ToMM(size.y), 4),
                      board.GetLayerName(footprint.GetLayer()))


def prepare_usb_routes(board):
    snapshot = ROOT / "outputs" / "before-usb-review.kicad_pcb"
    if not snapshot.exists():
        pcbnew.SaveBoard(str(snapshot), board)
    footprints = {footprint.GetReference(): footprint for footprint in board.GetFootprints()}
    diode = footprints["D1"]
    diode.SetPosition(pcbnew.VECTOR2I(pcbnew.FromMM(100), pcbnew.FromMM(109)))
    diode.SetOrientationDegrees(90)
    footprints["R5"].SetPosition(pcbnew.VECTOR2I(pcbnew.FromMM(96.7), pcbnew.FromMM(109)))
    footprints["R6"].SetPosition(pcbnew.VECTOR2I(pcbnew.FromMM(100.85), pcbnew.FromMM(104.4)))
    footprints["R7"].SetPosition(pcbnew.VECTOR2I(pcbnew.FromMM(99.15), pcbnew.FromMM(104.4)))
    for item in list(board.GetTracks()):
        board.Delete(item)

    paths = [
        ("USB_DP", pcbnew.B_Cu, 0.2, [(100.85, 103.575), (100.85, 102.85)]),
        ("USB_DP", pcbnew.F_Cu, 0.2, [(100.85, 102.85), (100.45, 102.45), (99.15, 102.45), (99.05, 102.35)]),
        ("USB_DP", pcbnew.B_Cu, 0.2, [(99.05, 102.35), (99.15, 102.25), (99.15, 101.83)]),
        ("USB_DM", pcbnew.B_Cu, 0.2, [(99.15, 103.575), (99.15, 103.3), (100, 102.45), (100, 101.83)]),
        ("USB_DP_CONN", pcbnew.B_Cu, 0.25, [(100.85, 105.225), (100.85, 105.8),
         (100.95, 105.9), (100.95, 107.8625), (100.95, 110.1375)]),
        ("USB_DM_CONN", pcbnew.B_Cu, 0.25, [(99.15, 105.225), (99.15, 105.8),
         (99.05, 105.9), (99.05, 107.8625), (99.05, 110.1375)]),
        ("USB_DP_CONN", pcbnew.B_Cu, 0.15, [(100.25, 115.25), (100.25, 114.4),
         (100.4, 114.25), (100.4, 111.75), (100.95, 111.2), (100.95, 110.1375)]),
        ("USB_DM_CONN", pcbnew.B_Cu, 0.15, [(99.75, 115.25), (99.75, 114.4),
         (99.6, 114.25), (99.6, 111.75), (99.05, 111.2), (99.05, 110.1375)]),
    ]
    for name, layer, width, positions in paths:
        for start, end in zip(positions, positions[1:]):
            track = pcbnew.PCB_TRACK(board)
            track.SetStart(pcbnew.VECTOR2I(pcbnew.FromMM(start[0]), pcbnew.FromMM(start[1])))
            track.SetEnd(pcbnew.VECTOR2I(pcbnew.FromMM(end[0]), pcbnew.FromMM(end[1])))
            track.SetLayer(layer)
            track.SetWidth(pcbnew.FromMM(width))
            track.SetNetCode(board.FindNet(name).GetNetCode())
            track.SetLocked(True)
            board.Add(track)
    for name, position in [("USB_DP", (100.85, 102.85)), ("USB_DP", (99.05, 102.35)),
                           ("GND", (101.9, 102.9)), ("GND", (98.0, 103.0))]:
        item = pcbnew.PCB_VIA(board)
        item.SetPosition(pcbnew.VECTOR2I(pcbnew.FromMM(position[0]), pcbnew.FromMM(position[1])))
        item.SetWidth(pcbnew.FromMM(0.6))
        item.SetDrill(pcbnew.FromMM(0.3))
        item.SetViaType(pcbnew.VIATYPE_THROUGH)
        item.SetLayerPair(pcbnew.F_Cu, pcbnew.B_Cu)
        item.SetNetCode(board.FindNet(name).GetNetCode())
        item.SetLocked(True)
        board.Add(item)
    pcbnew.SaveBoard(str(BOARD), board)
    print("Prepared locked USB routes and flow-through ESD placement; other nets require routing")


def repair_local_routes(board):
    report = json.loads((ROOT / "outputs" / "routed-drc.json").read_text(encoding="utf-8"))
    redundant = {item["uuid"] for violation in report["violations"]
                 if violation["type"] == "via_dangling" for item in violation["items"]}
    for item in list(board.GetTracks()):
        if not item.IsLocked() or item.GetNetname() in {"VBUS", "CC2"} or item.m_Uuid.AsString() in redundant:
            board.Delete(item)
    old = pcbnew.VECTOR2I(pcbnew.FromMM(99.5891), pcbnew.FromMM(119.3145))
    new = pcbnew.VECTOR2I(pcbnew.FromMM(100.2), pcbnew.FromMM(118.7))
    for item in board.GetTracks():
        if item.GetNetname() != "USB_DM_CONN":
            continue
        if isinstance(item, pcbnew.PCB_VIA):
            if (item.GetPosition() - old).SquaredEuclideanNorm() < pcbnew.FromMM(0.001) ** 2:
                item.SetPosition(new)
        else:
            if (item.GetStart() - old).SquaredEuclideanNorm() < pcbnew.FromMM(0.001) ** 2:
                item.SetStart(new)
            if (item.GetEnd() - old).SquaredEuclideanNorm() < pcbnew.FromMM(0.001) ** 2:
                item.SetEnd(new)
    paths = [
        ("VBUS", pcbnew.B_Cu, 0.28, [(101.25, 116.75), (101.25, 115.25), (101.25, 112.6)]),
        ("VBUS", pcbnew.B_Cu, 0.5, [(101.25, 112.6), (101.45, 112.4), (106.3, 112.4)]),
        ("VBUS", pcbnew.B_Cu, 0.28, [(98.75, 116.75), (98.75, 115.25), (98.75, 112.6)]),
        ("VBUS", pcbnew.F_Cu, 0.5, [(98.75, 112.6), (100, 112.6), (101.25, 112.6)]),
        ("VBUS", pcbnew.F_Cu, 0.5, [(100, 112.6), (100, 108.9)]),
        ("VBUS", pcbnew.B_Cu, 0.28, [(100, 108.9), (100, 107.8625)]),
        ("CC2", pcbnew.B_Cu, 0.15, [(97.525, 109), (97.525, 109.8), (96.8, 110.525),
         (96.8, 112), (95, 113.8), (95, 117.7), (95.6, 118.3), (96.4, 118.3)]),
        ("CC2", pcbnew.F_Cu, 0.15, [(96.4, 118.3), (96.4, 118.9), (96.6, 119.1),
         (98.8, 119.1), (99.25, 118.65)]),
        ("CC2", pcbnew.B_Cu, 0.15, [(99.25, 118.65), (99.25, 116.75)]),
    ]
    for name, layer, width, positions in paths:
        for start, end in zip(positions, positions[1:]):
            track = pcbnew.PCB_TRACK(board)
            track.SetStart(pcbnew.VECTOR2I(pcbnew.FromMM(start[0]), pcbnew.FromMM(start[1])))
            track.SetEnd(pcbnew.VECTOR2I(pcbnew.FromMM(end[0]), pcbnew.FromMM(end[1])))
            track.SetLayer(layer)
            track.SetWidth(pcbnew.FromMM(width))
            track.SetNetCode(board.FindNet(name).GetNetCode())
            track.SetLocked(True)
            board.Add(track)
    for name, position in [("VBUS", (98.75, 112.6)), ("VBUS", (101.25, 112.6)),
                           ("VBUS", (100, 108.9)), ("CC2", (96.4, 118.3)), ("CC2", (99.25, 118.65))]:
        via = pcbnew.PCB_VIA(board)
        via.SetPosition(pcbnew.VECTOR2I(pcbnew.FromMM(position[0]), pcbnew.FromMM(position[1])))
        via.SetWidth(pcbnew.FromMM(0.6))
        via.SetDrill(pcbnew.FromMM(0.3))
        via.SetViaType(pcbnew.VIATYPE_THROUGH)
        via.SetLayerPair(pcbnew.F_Cu, pcbnew.B_Cu)
        via.SetNetCode(board.FindNet(name).GetNetCode())
        via.SetLocked(True)
        board.Add(via)
    for footprint in board.GetFootprints():
        if footprint.GetReference() in {"U1", "U2", "D1"}:
            for pad in footprint.Pads():
                if pad.GetNetname() == "GND":
                    pad.SetLocalZoneConnection(pcbnew.ZONE_CONNECTION_FULL)
    pcbnew.SaveBoard(str(BOARD), board)
    print("Repaired VBUS/CC2 routing, edge clearance, redundant vias and module ground-pad connections")


def finish_cc1(board):
    moves = {
        "VBUS": [((101.25, 112.6), (101.35, 112.6))],
        "BUCK_BST": [((91.9194, 105.6444), (91.9194, 105.5444)),
                     ((89.7681, 105.6444), (89.7681, 105.5444))],
        "PIXEL_6": [((91.4206, 92.6547), (91.4206, 92.55)),
                    ((90.4038, 92.6547), (90.4038, 92.55))],
    }
    for item in list(board.GetTracks()):
        if item.GetNetname() == "CC1":
            board.Delete(item)
            continue
        for old_position, new_position in moves.get(item.GetNetname(), []):
            old = pcbnew.VECTOR2I(pcbnew.FromMM(old_position[0]), pcbnew.FromMM(old_position[1]))
            new = pcbnew.VECTOR2I(pcbnew.FromMM(new_position[0]), pcbnew.FromMM(new_position[1]))
            if isinstance(item, pcbnew.PCB_VIA):
                if (item.GetPosition() - old).SquaredEuclideanNorm() < pcbnew.FromMM(0.001) ** 2:
                    item.SetPosition(new)
            else:
                if (item.GetStart() - old).SquaredEuclideanNorm() < pcbnew.FromMM(0.001) ** 2:
                    item.SetStart(new)
                if (item.GetEnd() - old).SquaredEuclideanNorm() < pcbnew.FromMM(0.001) ** 2:
                    item.SetEnd(new)
    paths = [
        (pcbnew.B_Cu, [(95.825, 111.2), (96, 111.025), (96, 110.3)]),
        (pcbnew.F_Cu, [(96, 110.3), (96, 106.5), (98.45, 106.5),
         (100.95, 109), (102.8, 110.8), (102.8, 111.3)]),
        (pcbnew.B_Cu, [(102.8, 111.3), (101.65, 111.3), (100.75, 112.2), (100.75, 115.25)]),
    ]
    for layer, positions in paths:
        for start, end in zip(positions, positions[1:]):
            track = pcbnew.PCB_TRACK(board)
            track.SetStart(pcbnew.VECTOR2I(pcbnew.FromMM(start[0]), pcbnew.FromMM(start[1])))
            track.SetEnd(pcbnew.VECTOR2I(pcbnew.FromMM(end[0]), pcbnew.FromMM(end[1])))
            track.SetLayer(layer)
            track.SetWidth(pcbnew.FromMM(0.15))
            track.SetNetCode(board.FindNet("CC1").GetNetCode())
            board.Add(track)
    for position in [(96, 110.3), (102.8, 111.3)]:
        via = pcbnew.PCB_VIA(board)
        via.SetPosition(pcbnew.VECTOR2I(pcbnew.FromMM(position[0]), pcbnew.FromMM(position[1])))
        via.SetWidth(pcbnew.FromMM(0.6))
        via.SetDrill(pcbnew.FromMM(0.3))
        via.SetViaType(pcbnew.VIATYPE_THROUGH)
        via.SetLayerPair(pcbnew.F_Cu, pcbnew.B_Cu)
        via.SetNetCode(board.FindNet("CC1").GetNetCode())
        board.Add(via)
    for footprint in board.GetFootprints():
        if footprint.GetReference() in {"R10", "J1"}:
            for pad in footprint.Pads():
                if pad.GetNetname() == "GND":
                    pad.SetLocalZoneConnection(pcbnew.ZONE_CONNECTION_FULL)
    pcbnew.SaveBoard(str(BOARD), board)
    print("Completed CC1 and corrected the measured power-net clearance misses")


def stitch_ground(board):
    polygons = {layer: [] for layer in [pcbnew.F_Cu, pcbnew.B_Cu]}
    for zone in board.Zones():
        if not zone.GetIsRuleArea() and zone.GetNetname() == "GND":
            for layer in polygons:
                if zone.GetLayerSet().Contains(layer):
                    shape = zone.GetFilledPolysList(layer)
                    polygons[layer].extend((shape, index) for index in range(shape.OutlineCount()))
    joined = set()
    added = 0
    occupied = [item.GetPosition() for item in board.GetTracks() if isinstance(item, pcbnew.PCB_VIA)]
    holes = [(pad.GetPosition(), pcbnew.ToMM(max(pad.GetDrillSize().x, pad.GetDrillSize().y)) / 2 + 0.4)
             for footprint in board.GetFootprints() for pad in footprint.Pads()
             if pad.GetAttribute() in {pcbnew.PAD_ATTRIB_PTH, pcbnew.PAD_ATTRIB_NPTH}]
    offsets = [(0, 0), (0.31, 0), (-0.31, 0), (0, 0.31), (0, -0.31),
               (0.22, 0.22), (-0.22, 0.22), (0.22, -0.22), (-0.22, -0.22)]
    for grid_x in range(330, 472):
        for grid_y in range(350, 474):
            position_x, position_y = grid_x / 4, grid_y / 4
            sample = [pcbnew.VECTOR2I(pcbnew.FromMM(position_x + offset_x),
                      pcbnew.FromMM(position_y + offset_y)) for offset_x, offset_y in offsets]
            if any((sample[0] - position).SquaredEuclideanNorm() < pcbnew.FromMM(0.8) ** 2 for position in occupied):
                continue
            if any((sample[0] - position).SquaredEuclideanNorm() < pcbnew.FromMM(radius) ** 2 for position, radius in holes):
                continue
            candidates = []
            for layer in [pcbnew.F_Cu, pcbnew.B_Cu]:
                candidates.append([index for index, (shape, region) in enumerate(polygons[layer])
                                   if all(shape.Contains(point, region) for point in sample)])
            if not candidates[0] or not candidates[1]:
                continue
            pair = (candidates[0][0], candidates[1][0])
            if pair in joined:
                continue
            joined.add(pair)
            via = pcbnew.PCB_VIA(board)
            via.SetPosition(sample[0])
            via.SetWidth(pcbnew.FromMM(0.6))
            via.SetDrill(pcbnew.FromMM(0.3))
            via.SetViaType(pcbnew.VIATYPE_THROUGH)
            via.SetLayerPair(pcbnew.F_Cu, pcbnew.B_Cu)
            via.SetNetCode(board.FindNet("GND").GetNetCode())
            board.Add(via)
            occupied.append(sample[0])
            added += 1
    pcbnew.SaveBoard(str(BOARD), board)
    print(f"Added {added} ground stitching vias in clear overlapping filled copper regions")


def open_ground_corridor(board):
    for item in list(board.GetTracks()):
        if isinstance(item, pcbnew.PCB_VIA) and item.GetNetname() == "GND":
            position = item.GetPosition()
            if any(abs(pcbnew.ToMM(position.x) - position_x) < 0.001 and
                   abs(pcbnew.ToMM(position.y) - position_y) < 0.001
                   for position_x, position_y in [(97.5, 118), (99.7, 111), (99.59, 110.9), (99.59, 111.2)]):
                board.Delete(item)
        elif not isinstance(item, pcbnew.PCB_VIA) and item.GetNetname() == "VBUS" and item.GetLayer() == pcbnew.F_Cu:
            points = [item.GetStart(), item.GetEnd()]
            if all(99.99 <= pcbnew.ToMM(point.x) <= 100.51 and 108.89 <= pcbnew.ToMM(point.y) <= 112.61 for point in points) and item.GetStart().y != item.GetEnd().y:
                board.Delete(item)
    positions = [(100, 108.9), (100.35, 109.25), (100.35, 112.6)]
    for start, end in zip(positions, positions[1:]):
        track = pcbnew.PCB_TRACK(board)
        track.SetStart(pcbnew.VECTOR2I(pcbnew.FromMM(start[0]), pcbnew.FromMM(start[1])))
        track.SetEnd(pcbnew.VECTOR2I(pcbnew.FromMM(end[0]), pcbnew.FromMM(end[1])))
        track.SetLayer(pcbnew.F_Cu)
        track.SetWidth(pcbnew.FromMM(0.5))
        track.SetNetCode(board.FindNet("VBUS").GetNetCode())
        board.Add(track)
    for item in board.GetTracks():
        if isinstance(item, pcbnew.PCB_VIA) or item.GetNetname() != "USB_DM_CONN":
            continue
        for old_position, new_position in [((99.6, 111.75), (99.6, 112.3)), ((99.05, 111.2), (99.05, 111.75))]:
            old = pcbnew.VECTOR2I(pcbnew.FromMM(old_position[0]), pcbnew.FromMM(old_position[1]))
            new = pcbnew.VECTOR2I(pcbnew.FromMM(new_position[0]), pcbnew.FromMM(new_position[1]))
            if (item.GetStart() - old).SquaredEuclideanNorm() < pcbnew.FromMM(0.001) ** 2:
                item.SetStart(new)
            if (item.GetEnd() - old).SquaredEuclideanNorm() < pcbnew.FromMM(0.001) ** 2:
                item.SetEnd(new)
    via = pcbnew.PCB_VIA(board)
    via.SetPosition(pcbnew.VECTOR2I(pcbnew.FromMM(99.59), pcbnew.FromMM(111.2)))
    via.SetWidth(pcbnew.FromMM(0.6))
    via.SetDrill(pcbnew.FromMM(0.3))
    via.SetViaType(pcbnew.VIATYPE_THROUGH)
    via.SetLayerPair(pcbnew.F_Cu, pcbnew.B_Cu)
    via.SetNetCode(board.FindNet("GND").GetNetCode())
    board.Add(via)
    pcbnew.SaveBoard(str(BOARD), board)
    print("Opened a ground-via corridor and removed the shield-hole collision")


def release_conflicting_routes(board):
    for item in list(board.GetTracks()):
        if item.GetNetname() in {"LED_5V", "PIXEL_4", "BUCK_BST"}:
            board.Delete(item)
        elif item.GetNetname() == "CC1":
            item.SetLocked(True)
    pcbnew.SaveBoard(str(BOARD), board)
    print("Reserved CC1 and released the three conflicting surrounding nets for local rerouting")


def replace_board_connector(board):
    from check_circuit import child, form, set_property, write_tree
    import sexpdata

    footprint_name = "USB_C_Receptacle_HRO_TYPE-C-31-M-12"
    native = Path("C:/Program Files/KiCad/10.0/share/kicad/footprints/Connector_USB.pretty") / f"{footprint_name}.kicad_mod"
    definition = sexpdata.loads(native.read_text(encoding="utf-8"))
    model = child(definition, "model")
    model[1] = f"${{KIPRJMOD}}/LampParts.3dshapes/{footprint_name}.step"
    child(model, "offset")[:] = form('(offset (xyz 0 -1.05 0))')
    child(model, "rotate")[:] = form('(rotate (xyz 0 0 180))')
    set_property(definition, "Datasheet", "https://jlcpcb.com/api/file/downloadByFileSystemAccessId/8550723676069908480")
    write_tree(ROOT / "LampParts.pretty" / f"{footprint_name}.kicad_mod", definition)
    shutil.copyfile(ROOT / "outputs/connector-source.3dshapes/USB-C_SMD-TYPE-C-31-M-12_1.step",
                    ROOT / "LampParts.3dshapes" / f"{footprint_name}.step")
    footprints = {footprint.GetReference(): footprint for footprint in board.GetFootprints()}
    old = footprints["J1"]
    if old.GetValue() == "TYPE-C-31-M-12":
        raise ValueError("Right-angle connector already placed; refusing to replace routing twice")
    backup = ROOT / "outputs/before-right-angle.kicad_pcb"
    if not backup.exists():
        pcbnew.SaveBoard(str(backup), board)
    netlist = ET.parse(ROOT / "outputs/right-angle-netlist.xml")
    connections = {node.attrib["pin"]: net.attrib["name"].removeprefix("/")
                   for net in netlist.findall("./nets/net") for node in net.findall("node")
                   if node.attrib["ref"] == "J1"}
    replacement = pcbnew.FootprintLoad(str(ROOT / "LampParts.pretty"), footprint_name)
    if replacement is None:
        raise ValueError("Cannot load the verified right-angle footprint")
    replacement.SetReference("J1")
    replacement.SetValue("TYPE-C-31-M-12")
    replacement.SetFPID(pcbnew.LIB_ID("LampParts", footprint_name))
    replacement.SetPath(old.GetPath())
    replacement.SetUuid(old.m_Uuid)
    for pad in replacement.Pads():
        name = connections.get(pad.GetNumber())
        if name and not name.startswith("unconnected-"):
            pad.SetNet(board.FindNet(name))
            if name == "GND":
                pad.SetLocalZoneConnection(pcbnew.ZONE_CONNECTION_FULL)
    board.Remove(old)
    board.Add(replacement)
    replacement.SetPosition(pcbnew.VECTOR2I(pcbnew.FromMM(100), pcbnew.FromMM(116)))
    replacement.Flip(replacement.GetPosition(), pcbnew.FLIP_DIRECTION_LEFT_RIGHT)
    replacement.Reference().SetVisible(False)
    replacement.Value().SetVisible(False)
    footprints["R4"].SetPosition(pcbnew.VECTOR2I(pcbnew.FromMM(92.8), pcbnew.FromMM(113.1)))
    footprints["F1"].SetPosition(pcbnew.VECTOR2I(pcbnew.FromMM(106.5), pcbnew.FromMM(111)))
    removed = 0
    for item in list(board.GetTracks()):
        box = item.GetBoundingBox()
        intersects = pcbnew.ToMM(box.GetRight()) >= 94.4 and pcbnew.ToMM(box.GetLeft()) <= 105.6 and pcbnew.ToMM(box.GetBottom()) >= 110.5
        affected = item.GetNetname() in {"USB_DP_CONN", "USB_DM_CONN", "CC1", "CC2", "VBUS", "+5V"}
        backside = isinstance(item, pcbnew.PCB_VIA) or item.GetLayer() == pcbnew.B_Cu
        if affected or (intersects and backside):
            board.Delete(item)
            removed += 1
    board.BuildConnectivity()
    pcbnew.SaveBoard(str(BOARD), board)
    print(f"Placed rear right-angle connector facing the board edge; released {removed} affected routing items")


def fit_right_angle_connector(board):
    tracks = list(board.GetTracks())
    import sexpdata
    from check_circuit import child, tagged, write_tree

    path = ROOT / "LampParts.pretty/USB_C_Receptacle_HRO_TYPE-C-31-M-12.kicad_mod"
    definition = sexpdata.loads(path.read_text(encoding="utf-8"))
    definition[:] = [entry for entry in definition
                     if not (isinstance(entry, list) and str(entry[0]).startswith("fp_")
                             and child(entry, "layer") and child(entry, "layer")[1] == "F.SilkS")]
    write_tree(path, definition)
    footprints = {footprint.GetReference(): footprint for footprint in board.GetFootprints()}
    connector = footprints["J1"]
    connector.SetPosition(pcbnew.VECTOR2I(pcbnew.FromMM(100), pcbnew.FromMM(116.9)))
    for graphic in list(connector.GraphicalItems()):
        if graphic.GetLayer() == pcbnew.B_SilkS:
            connector.Remove(graphic)
    footprints["F1"].SetPosition(pcbnew.VECTOR2I(pcbnew.FromMM(106.45), pcbnew.FromMM(111)))
    for item in tracks:
        if item.GetNetname() == "EN":
            board.Delete(item)
    pcbnew.SaveBoard(str(BOARD), board)
    print("Positioned the horizontal connector at the board edge with LED/drill clearance")


def audit_connector(board):
    import sexpdata
    from check_circuit import child, children

    connector = next(footprint for footprint in board.GetFootprints() if footprint.GetReference() == "J1")
    assert connector.GetValue() == "TYPE-C-31-M-12"
    assert connector.GetFPID().GetUniStringLibId() == "LampParts:USB_C_Receptacle_HRO_TYPE-C-31-M-12"
    assert connector.GetLayer() == pcbnew.B_Cu
    assert abs(pcbnew.ToMM(connector.GetPosition().y) - 116.9) < 0.001
    definition = sexpdata.loads((ROOT / "LampParts.pretty/USB_C_Receptacle_HRO_TYPE-C-31-M-12.kicad_mod").read_text(encoding="utf-8"))
    locators = [pad for pad in children(definition, "pad") if str(pad[2]) == "np_thru_hole"]
    assert len(locators) == 2
    positions = sorted((float(child(pad, "at")[1]), float(child(pad, "at")[2])) for pad in locators)
    assert positions == [(-2.89, -2.6), (2.89, -2.6)]
    assert all(abs(float(child(pad, "drill")[1]) - 0.65) < 0.001 for pad in locators)
    anchors = [pad for pad in children(definition, "pad") if pad[1] == "SH"]
    assert len(anchors) == 4 and all(str(pad[2]) == "thru_hole" for pad in anchors)
    model = child(definition, "model")
    assert list(child(child(model, "offset"), "xyz")[1:]) == [0, -1.05, 0]
    assert list(child(child(model, "rotate"), "xyz")[1:]) == [0, 0, 180]
    model_path = Path(str(model[1]).replace("${KIPRJMOD}", str(ROOT)))
    assert model_path.is_file() and model_path.stat().st_size > 1000
    assert all(pad.GetNetname() == "GND" for pad in connector.Pads() if pad.GetNumber() == "SH")
    print("PASS: rear side-entry M12, four grounded shield slots, two 0.65 mm locator holes, peg-aligned STEP transform")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--repair-ground-pads", action="store_true")
    parser.add_argument("--zones", action="store_true")
    parser.add_argument("--netlist", type=Path)
    parser.add_argument("--dsn", type=Path)
    parser.add_argument("--ses", type=Path)
    parser.add_argument("--routing", action="store_true")
    parser.add_argument("--pads", action="store_true")
    parser.add_argument("--prepare-usb", action="store_true")
    parser.add_argument("--repair-local", action="store_true")
    parser.add_argument("--edge-inset", type=float, default=0)
    parser.add_argument("--finish-cc1", action="store_true")
    parser.add_argument("--stitch-ground", action="store_true")
    parser.add_argument("--release-conflicts", action="store_true")
    parser.add_argument("--ground-corridor", action="store_true")
    parser.add_argument("--replace-connector", action="store_true")
    parser.add_argument("--fit-right-angle", action="store_true")
    parser.add_argument("--connector", action="store_true")
    args = parser.parse_args()
    board = pcbnew.LoadBoard(str(BOARD))
    if args.replace_connector:
        replace_board_connector(board)
    if args.fit_right_angle:
        fit_right_angle_connector(board)
    if args.connector:
        audit_connector(board)
    if args.prepare_usb:
        prepare_usb_routes(board)
    if args.repair_local:
        repair_local_routes(board)
    if args.finish_cc1:
        finish_cc1(board)
    if args.stitch_ground:
        stitch_ground(board)
    if args.release_conflicts:
        release_conflicting_routes(board)
    if args.ground_corridor:
        open_ground_corridor(board)
    if args.repair_ground_pads:
        repair_module_ground(board)
        pcbnew.SaveBoard(str(BOARD), board)
        print("Module corner pads 62-65 match the official shared MINI land pattern and connect to GND")
    if args.zones:
        add_ground_zones(board)
        pcbnew.SaveBoard(str(BOARD), board)
        print("Added GND pours on F.Cu and B.Cu; native CLI refill required")
    if args.netlist:
        audit(board, args.netlist)
    if args.routing:
        report_routing(board)
    if args.pads:
        report_pads(board)
    if args.dsn:
        if args.edge_inset:
            edges = [drawing for drawing in board.GetDrawings() if drawing.GetLayer() == pcbnew.Edge_Cuts]
            if len(edges) != 1:
                raise ValueError("Routing inset expects the single circular board outline")
            edges[0].SetEnd(pcbnew.VECTOR2I(pcbnew.FromMM(120 - args.edge_inset), pcbnew.FromMM(100)))
        if not pcbnew.ExportSpecctraDSN(board, str(args.dsn)):
            raise RuntimeError("Native Specctra export failed")
        print(f"Exported {args.dsn}")
    if args.ses:
        if not pcbnew.ImportSpecctraSES(board, str(args.ses)):
            raise RuntimeError("Native Specctra import failed")
        pcbnew.SaveBoard(str(BOARD), board)
        print(f"Imported {args.ses}")