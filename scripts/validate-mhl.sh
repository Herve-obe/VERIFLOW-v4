#!/usr/bin/env bash
# Valide les MHL produits par VERIFLOW avec l'implémentation de référence de l'ASC
# (https://github.com/ascmitc/mhl). Usage : scripts/validate-mhl.sh <dossier copié>
# Prérequis : python3 ; l'outil est installé dans un environnement virtuel temporaire.
set -euo pipefail
ROOT="${1:?dossier copié (contenant ascmhl/)}"
WORK="${MHL_WORK:-$HOME/.cache/veriflow-ascmhl}"
if [ ! -x "$WORK/env/bin/ascmhl" ]; then
  git clone -q --depth 1 https://github.com/ascmitc/mhl "$WORK/mhl"
  python3 -m venv "$WORK/env"
  "$WORK/env/bin/pip" install -q -e "$WORK/mhl"
fi
cd "$WORK/mhl"
for f in "$ROOT"/ascmhl/*.mhl; do "$WORK/env/bin/ascmhl-debug" xsd-schema-check "$f"; done
"$WORK/env/bin/ascmhl-debug" xsd-schema-check -df "$ROOT/ascmhl/ascmhl_chain.xml"
"$WORK/env/bin/ascmhl-debug" verify "$ROOT" && echo "Vérification ASC MHL : OK"
