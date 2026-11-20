import argparse

import sexpdata

import check_circuit as source


BOARD = source.ROOT / "TPL01B.kicad_pcb"


def set_part(references, value, mpn, lcsc, datasheet):
    import pcbnew

    fields = {"MPN": mpn, "LCSC": lcsc}
    if datasheet:
        fields["Datasheet"] = datasheet
    document = sexpdata.loads(source.SCHEMATIC.read_text(encoding="utf-8"))
    found = set()
    for symbol in source.children(document, "symbol"):
        properties = source.properties(symbol)
        reference = str(properties["Reference"][2])
        if reference not in references:
            continue
        found.add(reference)
        properties["Value"][2] = value
        for name, text in fields.items():
            source.set_property(symbol, name, text)
    if found != set(references):
        raise ValueError(f"Not in the schematic: {sorted(set(references) - found)}")
    board = pcbnew.LoadBoard(str(BOARD))
    for reference in references:
        footprint = board.FindFootprintByReference(reference)
        footprint.SetValue(value)
        for name, text in fields.items():
            if not footprint.HasField(name):
                raise ValueError(f"{reference} footprint has no {name} field")
            footprint.SetField(name, text)
    source.write_tree(source.SCHEMATIC, document)
    pcbnew.SaveBoard(str(BOARD), board)
    print(f"Set {len(references)} parts to {mpn} / {lcsc} in schematic and board")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(
        description="Change the purchasing identity of existing parts; close the KiCad editors first")
    parser.add_argument("--refs", required=True, help="Comma-separated references")
    parser.add_argument("--value", required=True)
    parser.add_argument("--mpn", required=True)
    parser.add_argument("--lcsc", required=True)
    parser.add_argument("--datasheet")
    args = parser.parse_args()
    set_part(args.refs.split(","), args.value, args.mpn, args.lcsc, args.datasheet)
