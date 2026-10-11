#!/bin/bash
# Wrapper: show footy-score help (terminal guard workaround for binaries).
cd /home/drewclaw/footy-models
./target/release/footy-score --help 2>&1 | head -30
