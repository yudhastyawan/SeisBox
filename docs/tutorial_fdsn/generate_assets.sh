#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" &> /dev/null && pwd)"
PROJECT_DIR="$(dirname "$(dirname "$SCRIPT_DIR")")"
FDSN_EXE="$PROJECT_DIR/target/release/seisbox_fdsn"

cd "$SCRIPT_DIR"

echo "Building release binary..."
cd "$PROJECT_DIR"
cargo build --release --bin seisbox_fdsn
cd "$SCRIPT_DIR"

echo "1. Running FDSN Earthquake Extraction (Turkey M7.8)"
"$FDSN_EXE" \
    --providers KOERI \
    --lat 37.174 \
    --lon 37.032 \
    --min-radius 0 --max-radius 2.0 \
    --ref-time "2023-02-06 01:17:35" \
    --start-offset -60 \
    --end-offset 300 \
    --channel "HHZ,BHZ" \
    --plot-map fdsn_turkey_map.png \
    --out-dir "$SCRIPT_DIR/fdsn_data"

echo "2. Running FDSN Advanced Extraction (Custom KOERI Node + Wildcard + SAC Export + QuakeML Phase)"
"$FDSN_EXE" \
    --providers "KOERI_CUSTOM:http://eida.koeri.boun.edu.tr" \
    --lat 37.174 \
    --lon 37.032 \
    --min-radius 0 --max-radius 2.0 \
    --start-time "2023-02-06 01:17:35" --end-time "2023-02-06 01:18:35" \
    --channel "HH*,BH*" \
    --export-sac \
    --download-events \
    --download-arrivals \
    --download-xml \
    --min-mag 7.0 \
    --out-dir "$SCRIPT_DIR/fdsn_data_custom"

echo "3. Running FDSN Event & Arrival Extraction (GFZ)"
"$FDSN_EXE" \
    --providers "GFZ:http://geofon.gfz-potsdam.de" \
    --lat 37.174 \
    --lon 37.032 \
    --min-radius 0 --max-radius 5.0 \
    --start-time "2023-02-06 00:00:00" --end-time "2023-02-06 23:59:59" \
    --channel "HH*" \
    --min-mag 7.0 \
    --download-events \
    --download-arrivals \
    --download-xml \
    --out-dir "$SCRIPT_DIR/fdsn_data_gfz"

echo "4. Building PDF with Pandoc"
cp ../tutorial_cfs/custom_header.tex .
pandoc tutorial_fdsn.md -o SeisBox_FDSN_Manual.pdf -V geometry:margin=1in --toc --syntax-highlighting=tango --include-in-header=custom_header.tex

echo "Done generating assets and PDF!"
