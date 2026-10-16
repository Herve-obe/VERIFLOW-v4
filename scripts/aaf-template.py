#!/usr/bin/env python3
"""Génère core/src/aaf/template.bin : le métadictionnaire et le dictionnaire
d'un fichier AAF vide, tels qu'écrits par pyaaf2 (MIT, Mark Reid,
https://github.com/markreid/pyaaf2).

Ces deux sous-arbres ne dépendent pas du contenu exporté : VERIFLOW les recopie
tels quels et écrit lui-même l'en-tête, les mobs et la table des références.

Usage (une seule fois, ou après une mise à jour de pyaaf2) :
    python3 -m venv /tmp/aafenv && /tmp/aafenv/bin/pip install pyaaf2==1.7.1
    /tmp/aafenv/bin/python scripts/aaf-template.py core/src/aaf/template.bin

Format de sortie (petit-boutiste) :
    b"VFAAFT1\\0"
    u32 nombre de chemins de la table des références, puis pour chacun :
        u16 nombre de pids, u16 pids...
    u32 nombre d'entrées, puis pour chacune :
        u8 type (0 stockage, 1 flux), u16 longueur, chemin UTF-8 depuis la racine,
        stockage : CLSID 16 octets ; flux : u32 longueur, données.
"""

import io
import struct
import sys
import tempfile
import os

import aaf2
from aaf2.cfb import CompoundFileBinary

KEEP = ("MetaDictionary-1", "Header-2/Dictionary-3b04")


def main(out):
    with tempfile.TemporaryDirectory() as tmp:
        path = os.path.join(tmp, "vide.aaf")
        with aaf2.open(path, "w"):
            pass
        with open(path, "rb") as fh:
            cfb = CompoundFileBinary(fh, "rb")
            buf = io.BytesIO()
            buf.write(b"VFAAFT1\0")

            # Table des références faibles (chemins de pids).
            data = cfb.open("/referenced properties").read()
            assert data[0] == 0x4C
            count, pids = struct.unpack("<HI", data[1:7])
            table, path_pids = [], []
            for (pid,) in struct.iter_unpack("<H", data[7:7 + 2 * pids]):
                if pid:
                    path_pids.append(pid)
                else:
                    table.append(path_pids)
                    path_pids = []
            assert len(table) == count
            buf.write(struct.pack("<I", len(table)))
            for p in table:
                buf.write(struct.pack("<H", len(p)))
                buf.write(struct.pack("<%dH" % len(p), *p))

            entries = []

            def walk(entry, rel):
                entries.append((0, rel, entry.class_id.bytes_le))
                for child in sorted(cfb.listdir(entry), key=lambda e: e.name):
                    name = rel + "/" + child.name
                    if child.isdir():
                        walk(child, name)
                    else:
                        entries.append((1, name, cfb.open("/" + name).read()))

            for keep in KEEP:
                walk(cfb.find("/" + keep), keep)

            buf.write(struct.pack("<I", len(entries)))
            for kind, name, payload in entries:
                raw = name.encode("utf-8")
                buf.write(struct.pack("<BH", kind, len(raw)))
                buf.write(raw)
                if kind == 1:
                    buf.write(struct.pack("<I", len(payload)))
                buf.write(payload)

    with open(out, "wb") as fh:
        fh.write(buf.getvalue())
    print("%s : %d chemins, %d entrées, %d octets" % (out, len(table), len(entries), len(buf.getvalue())))
    for i, p in enumerate(table):
        print("  référence %d : %s" % (i, ".".join("%04x" % x for x in p)))


if __name__ == "__main__":
    main(sys.argv[1])
