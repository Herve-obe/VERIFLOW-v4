#!/usr/bin/env python3
"""Contrôle externe des exports de logs du PLAYER avec OpenTimelineIO.

Usage : VERIFLOW_LOGS_DUMP=<dossier> cargo test -p veriflow-core --lib dump_for
        python3 scripts/validate-logs.py <dossier>

Nécessite : pip install opentimelineio otio-cmx3600-adapter otio-fcpx-xml-adapter otio-ale-adapter
Chaque fichier est relu par l'adaptateur OTIO correspondant ; le script
affiche les clips, leurs plages source et les marqueurs retrouvés.
"""
import sys
from pathlib import Path

import opentimelineio as otio


def show(path: Path, adapter: str, **kw) -> None:
    obj = otio.adapters.read_from_file(str(path), adapter_name=adapter, **kw)
    print(f"== {path.name} ({adapter})")
    for c in obj.find_clips():
        r = c.source_range
        src = f"{r.start_time.to_timecode()} +{int(r.duration.value)} a {r.start_time.rate:g} i/s" if r else "?"
        print(f"  clip {c.name!r} {src}")
        for m in c.markers:
            print(f"    marqueur {m.color} {m.marked_range.start_time.to_timecode()} {m.name!r}")


def main() -> None:
    d = Path(sys.argv[1])
    show(d / "logs.otio", "otio_json")
    # Une EDL ne porte pas sa cadence : celle de l'exemple est 25 i/s.
    show(d / "logs.edl", "cmx_3600", rate=25)
    show(d / "logs.fcpxml", "fcpx_xml")
    show(d / "logs.ale", "ale", fps=25)
    print("OK")


if __name__ == "__main__":
    main()
