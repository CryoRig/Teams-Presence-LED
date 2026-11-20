import argparse
import csv
import json
import math
import xml.etree.ElementTree as ET
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
BOARD = ROOT / "teams-presence-led.kicad_pcb"
EASYEDA_UNIT_MM = 0.254
MAX_PAD_ERROR_MM = 0.6


def fetch(review):
    from easyeda2kicad.easyeda.easyeda_api import EasyedaApi

    api = EasyedaApi()
    netlist = ET.parse(review / "netlist.xml")
    parts = {field.text for field in netlist.iterfind(".//comp/fields/field[@name='LCSC']")}
    library = {}
    for lcsc in sorted(parts):
        package = (api.get_cad_data_of_component(lcsc_id=lcsc) or {}).get("packageDetail")
        if not package:
            raise ValueError(f"JLCPCB/EasyEDA has no footprint for {lcsc}")
        data = package["dataStr"]
        origin_x, origin_y = float(data["head"]["x"]), float(data["head"]["y"])
        pads = []
        for shape in data["shape"]:
            fields = shape.split("~")
            if fields[0] == "PAD":
                pads.append({"number": fields[8],
                             "x": round((float(fields[2]) - origin_x) * EASYEDA_UNIT_MM, 4),
                             "y": round((float(fields[3]) - origin_y) * EASYEDA_UNIT_MM, 4),
                             "through": fields[6] == "11"})
        if not pads:
            raise ValueError(f"No pads in the JLCPCB/EasyEDA footprint for {lcsc}")
        library[lcsc] = {"footprint": package["title"], "pads": pads}
    (review / "jlc-footprints.json").write_text(json.dumps(library, indent=1), encoding="utf-8")
    print(f"Saved {len(library)} JLCPCB library footprints")


def unique_pads(pads):
    numbers = [pad[0] for pad in pads]
    return {pad[0]: pad for pad in pads if pad[0] and numbers.count(pad[0]) == 1}


def mean_shift(pairs):
    return (sum(placed[1] - turned[1] for placed, turned in pairs) / len(pairs),
            sum(placed[2] - turned[2] for placed, turned in pairs) / len(pairs))


def nearest(pad, shift, candidates):
    return min(candidates, key=lambda other: math.hypot(other[1] - pad[1] - shift[0], other[2] - pad[2] - shift[1]))


def gap(pad, shift, other):
    return math.hypot(other[1] - pad[1] - shift[0], other[2] - pad[2] - shift[1])


def fit(review):
    import pcbnew

    library = json.loads((review / "jlc-footprints.json").read_text(encoding="utf-8"))
    board = pcbnew.LoadBoard(str(BOARD))
    rows = []
    for footprint in sorted(board.GetFootprints(), key=lambda item: item.GetReference()):
        if footprint.GetAttributes() & pcbnew.FP_EXCLUDE_FROM_POS_FILES:
            continue
        reference = footprint.GetReference()
        part = library[footprint.GetFieldText("LCSC")]
        bottom = footprint.GetLayer() == pcbnew.B_Cu
        native = footprint.GetOrientationDegrees() % 360
        placed = [(pad.GetNumber(), pcbnew.ToMM(pad.GetPosition().x), pcbnew.ToMM(pad.GetPosition().y),
                   pad.GetAttribute() == pcbnew.PAD_ATTRIB_PTH)
                  for pad in footprint.Pads() if pad.GetAttribute() != pcbnew.PAD_ATTRIB_NPTH]
        candidates = []
        for quarter in range(4):
            angle = ((-native if bottom else native) % 90 + 90 * quarter) % 360
            cosine, sine = math.cos(math.radians(angle)), math.sin(math.radians(angle))
            turned = []
            for pad in part["pads"]:
                x = pad["x"] * cosine + pad["y"] * sine
                y = -pad["x"] * sine + pad["y"] * cosine
                # JLCPCB places a bottom part like a top part with this angle, mirrored left-right.
                turned.append((pad["number"], -x if bottom else x, y, pad["through"]))
            placed_unique, turned_unique = unique_pads(placed), unique_pads(turned)
            common = sorted(set(placed_unique) & set(turned_unique))
            if len(common) >= 2:
                pairs = [(placed_unique[number], turned_unique[number]) for number in common]
                shift = mean_shift(pairs)
                error = max(gap(turned_pad, shift, placed_pad) for placed_pad, turned_pad in pairs)
            else:
                shift = tuple((min(pad[axis] for pad in placed) + max(pad[axis] for pad in placed)
                               - min(pad[axis] for pad in turned) - max(pad[axis] for pad in turned)) / 2
                              for axis in (1, 2))
                error = max(gap(pad, shift, nearest(pad, shift, placed)) for pad in turned)
            candidates.append((error, angle, shift, turned))
        candidates.sort(key=lambda candidate: candidate[0])
        error, angle, shift, turned = candidates[0]
        if error > MAX_PAD_ERROR_MM or candidates[1][0] - error < 0.2:
            raise ValueError(f"{reference}: no unambiguous fit of {part['footprint']} "
                             f"(best {error:.3f} mm, next {candidates[1][0]:.3f} mm)")
        through = [pad for pad in turned if pad[3]]
        placed_through = [pad for pad in placed if pad[3]]
        if through and placed_through:
            # Through-hole anchors fix where the part sits; align the centre on them.
            shift = mean_shift([(nearest(pad, shift, placed_through), pad) for pad in through])
        residual = max(gap(pad, shift, nearest(pad, shift, placed)) for pad in turned)
        anchor = footprint.GetPosition()
        rows.append({
            "Ref": reference,
            "Val": footprint.GetValue(),
            "Package": str(footprint.GetFPID().GetLibItemName()),
            "PosX": f"{shift[0]:.6f}",
            "PosY": f"{-shift[1]:.6f}",
            "Rot": f"{angle:.6f}",
            "Side": "bottom" if bottom else "top",
            "NativeRot": f"{native:.1f}",
            "CentreShiftMm": f"{math.hypot(shift[0] - pcbnew.ToMM(anchor.x), shift[1] - pcbnew.ToMM(anchor.y)):.3f}",
            "PadResidualMm": f"{residual:.3f}",
            "JlcFootprint": part["footprint"],
        })
    with open(review / "jlc-placement.csv", "w", newline="", encoding="utf-8") as output:
        writer = csv.DictWriter(output, fieldnames=list(rows[0]), quoting=csv.QUOTE_ALL)
        writer.writeheader()
        writer.writerows(rows)
    changed = [row for row in rows if float(row["Rot"]) != float(row["NativeRot"]) or float(row["CentreShiftMm"]) > 0.05]
    print(f"PASS: {len(rows)} placements fitted to JLCPCB library footprints; "
          f"{len(changed)} differ from the native KiCad position file")
    for row in changed:
        print(f"  {row['Ref']:<4} {row['Side']:<6} rotation {row['NativeRot']:>5} -> {float(row['Rot']):>5.1f}, "
              f"centre moved {row['CentreShiftMm']} mm, pad residual {row['PadResidualMm']} mm ({row['JlcFootprint']})")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("review", type=Path, help="Review folder containing netlist.xml")
    parser.add_argument("--fetch", action="store_true",
                        help="Download JLCPCB library footprints (needs easyeda2kicad, not pcbnew)")
    parser.add_argument("--fit", action="store_true",
                        help="Write jlc-placement.csv from the saved board (needs pcbnew); does not modify the board")
    args = parser.parse_args()
    if args.fetch:
        fetch(args.review)
    if args.fit:
        fit(args.review)
    if not args.fetch and not args.fit:
        parser.error("Choose --fetch or --fit")
