#!/bin/bash

# 5. D2 ausführen und Ausgaben stummschalten (ohne < /dev/null, um stdin zu erhalten)
d2 --sketch --force-appendix "$@"
