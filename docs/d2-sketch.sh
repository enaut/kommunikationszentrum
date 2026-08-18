#!/bin/bash

# 1. Das letzte Argument ist immer der Ziel-Pfad (z.B. /.../d2/1.1.svg)
TGT_PATH="${@: -1}"

# 2. Reinen Dateinamen herausschneiden (z.B. 1.1.svg)
SVG_FILE=$(basename "$TGT_PATH")

# 4. Ausgabe direkt an das Terminal senden (z.B. 1.1.d2 → 1.1.svg)
echo "building graph: $SVG_FILE" > /dev/tty

# 5. D2 ausführen und Ausgaben stummschalten (ohne < /dev/null, um stdin zu erhalten)
d2 --sketch --force-appendix "$@" < /dev/null > /dev/null 2>&1
