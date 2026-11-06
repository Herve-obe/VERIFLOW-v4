#!/usr/bin/env python3
"""Contrôle externe des exports vers le montage avec OpenTimelineIO.

Usage : VERIFLOW_EXPORTS_DUMP=<dossier> cargo test -p veriflow-core --lib dump_for
        python3 scripts/validate-exports.py <dossier> [<dossier_logs>]

Nécessite : pip install opentimelineio otio-cmx3600-adapter otio-fcpx-xml-adapter otio-ale-adapter
(l'adaptateur fcp_xml, pour le XMEML, n'est pas publié sur PyPI : sans lui, le
XMEML est seulement vérifié comme XML bien formé, sans relecture de ses clips
ni de ses liens : c'est la limite de ce contrôle.)
L'adaptateur fcpx_xml ne lit que les projets (séquences) alors que nos FCPXML
n'ont qu'un événement : leur structure est donc contrôlée ici avec ElementTree
(bibliothèque, événement, asset-clip, références, au moins un clip).
<dossier> contient VERIFLOW.{otio,edl,fcpxml,xml,ale,csv} écrits par le test
`editorial::tests::dump_for_external_check`. Les logs du PLAYER (logs.*) sont
contrôlés de la même façon si <dossier_logs> est donné
(VERIFLOW_LOGS_DUMP, test `player::logs::dump_for_external_check`).
Chaque fichier est relu par l'adaptateur OTIO correspondant ; le script
affiche les clips, leurs plages source et les marqueurs retrouvés.
"""
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

import opentimelineio as otio


def clips_of(obj):
    return obj.find_clips() if hasattr(obj, "find_clips") else [
        c for child in obj for c in clips_of(child)
    ]


def tc(t) -> str:
    # Le son est compté en échantillons : pas de timecode SMPTE à 48000 i/s.
    try:
        return t.to_timecode()
    except ValueError:
        return f"{int(t.value)}@{t.rate:g}"


def show(path: Path, adapter: str, **kw) -> int:
    obj = otio.adapters.read_from_file(str(path), adapter_name=adapter, **kw)
    print(f"== {path.name} ({adapter})")
    n = 0
    for c in clips_of(obj):
        n += 1
        r = c.source_range
        src = f"{tc(r.start_time)} +{int(r.duration.value)} a {r.start_time.rate:g} i/s" if r else "?"
        print(f"  clip {c.name!r} {src}")
        for m in c.markers:
            print(f"    marqueur {m.color} {tc(m.marked_range.start_time)} {m.name!r}")
    return n


def check_fcpxml(path: Path) -> None:
    root = ET.parse(path).getroot()
    assert root.tag == "fcpxml", root.tag
    event = root.find("library/event")
    assert event is not None, "library/event absent"
    resources = root.find("resources")
    assert resources is not None, "resources absent"
    ids = {e.get("id") for e in resources if e.get("id")}
    for a in resources.findall("asset"):
        assert a.get("src") is None, f"asset {a.get('id')} : src (1.9+ : media-rep)"
        assert a.find("media-rep") is not None, f"asset {a.get('id')} sans media-rep"
    clips = event.findall(".//asset-clip")
    assert clips, "aucun asset-clip dans l'événement"
    for el in root.iter():
        for key in ("ref", "format"):
            v = el.get(key)
            if v is not None:
                assert v in ids, f"{el.tag} {key}={v} sans ressource"
    print(f"== {path.name} (structure FCPXML, {len(clips)} asset-clip, {len(ids)} ressources)")


def check_xmeml(path: Path) -> None:
    if "fcp_xml" in otio.adapters.available_adapter_names():
        show(path, "fcp_xml")
        return
    root = ET.parse(path).getroot()
    assert root.tag == "xmeml", root.tag
    print(f"== {path.name} (XML bien formé, {len(root.findall('.//clip'))} clips maîtres)")


def main() -> None:
    d = Path(sys.argv[1])
    # La collection OTIO regroupe tous les clips ; le clip synchronisé est une pile.
    assert show(d / "VERIFLOW.otio", "otio_json") == 3  # 2 clips + le son synchronisé
    # Une EDL ne porte pas sa cadence : celle de l'exemple est 25 i/s.
    show(d / "VERIFLOW.edl", "cmx_3600", rate=25)
    check_fcpxml(d / "VERIFLOW.fcpxml")
    check_xmeml(d / "VERIFLOW.xml")
    show(d / "VERIFLOW.ale", "ale", fps=25)
    if len(sys.argv) > 2:
        d = Path(sys.argv[2])
        show(d / "logs.otio", "otio_json")
        show(d / "logs.edl", "cmx_3600", rate=25)
        check_fcpxml(d / "logs.fcpxml")
        show(d / "logs.ale", "ale", fps=25)
    print("OK")


if __name__ == "__main__":
    main()
