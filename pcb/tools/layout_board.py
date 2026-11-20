import math
from pathlib import Path

import pcbnew


ROOT = Path(__file__).resolve().parents[1]
BOARD = ROOT / "teams-presence-led.kicad_pcb"
LIBRARIES = Path("C:/Program Files/KiCad/10.0/share/kicad/footprints")
POSITIONS = {
    "U1": (100, 94.8, 0), "J1": (100, 116, 0),
    "R1": (89.5, 89.0, 0), "C1": (89.5, 91.5, 0),
    "R2": (110.0, 92.5, 0), "R3": (90.3, 94.2, 0),
    "SW1": (111.0, 113.0, 0), "SW2": (113.5, 104.2, 0),
    "R4": (95.0, 111.2, 0), "R5": (98.0, 111.2, 0),
    "R6": (99.15, 104.4, 90), "R7": (100.85, 104.4, 90),
    "D1": (103.8, 106.0, 0), "F1": (106.3, 111.0, 90),
    "U2": (89.5, 108.0, 0), "C2": (89.2, 111.6, 0),
    "L1": (86.4, 104.0, 0), "C3": (93.6, 108.1, 0),
    "C4": (87.5, 99.4, 0), "C5": (87.5, 96.4, 0),
    "C6": (109.5, 89.4, 90), "U4": (110.2, 98.3, 0),
    "C8": (113.8, 98.3, 0), "C9": (114.3, 93.6, 0),
    "R10": (110.0, 95.3, 0), "R11": (113.8, 101.2, 0),
    "U3": (110.5, 108.1, 0), "R8": (108.0, 115.6, 0),
    "R9": (108.0, 104.0, 0), "C7": (114.6, 108.7, 0),
    "TP1": (98.5, 99, 0), "TP2": (101.5, 99, 0),
    "TP3": (98.5, 101, 0), "TP4": (101.5, 101, 0),
}
FRONT = {f"D{number}" for number in range(2, 10)} | {f"C{number}" for number in range(10, 18)} | {f"TP{number}" for number in range(1, 5)}
for index in range(8):
    angle = math.radians(-67.5 + index * 45)
    POSITIONS[f"D{index + 2}"] = (100 + 10.5 * math.cos(angle), 100 + 10.5 * math.sin(angle), 0)
    POSITIONS[f"C{index + 10}"] = (100 + 4.8 * math.cos(angle), 100 + 4.8 * math.sin(angle), 0)
POSITIONS["C12"] = (104.0, 100.0, 0)


def point(position_x, position_y):
    return pcbnew.VECTOR2I(pcbnew.FromMM(position_x), pcbnew.FromMM(position_y))


def apply_layout():
    board = pcbnew.LoadBoard(str(BOARD))
    if list(board.GetTracks()):
        raise ValueError("Refusing to overwrite an already routed board")
    previous_footprints = list(board.GetFootprints())
    if len(previous_footprints) != 50:
        raise ValueError("Expected the 50 schematic-derived footprints")
    for previous in previous_footprints:
        reference = previous.GetReference()
        identifier = previous.GetFPID().GetUniStringLibId()
        if reference in {"SW1", "SW2"}:
            identifier = "Button_Switch_SMD:SW_SPST_B3U-1000P"
        library, name = identifier.split(":", 1)
        directory = ROOT if library == "LampParts" else LIBRARIES
        footprint = pcbnew.FootprintLoad(str(directory / f"{library}.pretty"), name)
        if footprint is None:
            raise ValueError(f"Cannot load {identifier}")
        footprint.SetReference(reference)
        footprint.SetValue(previous.GetValue())
        footprint.SetFPID(pcbnew.LIB_ID(library, name))
        footprint.SetPath(previous.GetPath())
        footprint.SetUuid(previous.m_Uuid)
        nets = {pad.GetNumber(): pad.GetNet() for pad in previous.Pads() if pad.GetNetCode() > 0}
        for pad in footprint.Pads():
            number = pad.GetNumber()
            if number == "GND":
                pad.SetNumber("61")
                number = "61"
            if number in nets:
                pad.SetNet(nets[number])
        board.Remove(previous)
        board.Add(footprint)
        position_x, position_y, rotation = POSITIONS[reference]
        footprint.SetPosition(point(position_x, position_y))
        footprint.SetOrientationDegrees(rotation)
        if reference not in FRONT:
            footprint.Flip(footprint.GetPosition(), pcbnew.FLIP_DIRECTION_LEFT_RIGHT)
        footprint.Reference().SetVisible(False)
        footprint.Value().SetVisible(False)
        for graphic in footprint.GraphicalItems():
            if isinstance(graphic, pcbnew.PCB_TEXT) and graphic.GetLayer() in {pcbnew.F_SilkS, pcbnew.B_SilkS}:
                graphic.SetVisible(False)
    for drawing in board.GetDrawings():
        if isinstance(drawing, pcbnew.PCB_TEXT):
            if drawing.GetText() == "BOOT":
                drawing.SetPosition(point(115.0, 106.6))
            elif drawing.GetText() == "RESET":
                drawing.SetPosition(point(111.0, 110.6))
            elif drawing.GetText() in {str(index) for index in range(1, 9)}:
                angle = math.radians(-67.5 + (int(drawing.GetText()) - 1) * 45)
                drawing.SetPosition(point(100 + 16.5 * math.cos(angle), 100 + 16.5 * math.sin(angle)))
    pcbnew.SaveBoard(str(BOARD), board)
    print("Native KiCad placement: 50 footprints; 40 mm circle; front LEDs; rear module/USB")


if __name__ == "__main__":
    apply_layout()