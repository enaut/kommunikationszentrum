#!/bin/bash
echo "building graph"

# 1. D2 normal ausführen und Fehlermeldungen (stderr) unterdrücken
d2 --sketch "$@" 2>/dev/null
