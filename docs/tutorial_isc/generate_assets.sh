#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" &> /dev/null && pwd)"
PROJECT_DIR="$(dirname "$(dirname "$SCRIPT_DIR")")"
ISC_EXE="$PROJECT_DIR/target/release/seisbox_isc"

cd "$SCRIPT_DIR"

echo "Building release binary..."
cd "$PROJECT_DIR"
cargo build --release --bin seisbox_isc
cd "$SCRIPT_DIR"

echo "Creating conversion rules file..."
cat << 'EOF' > aturan_konversi.csv
SOURCE_TYPE,MIN_MAG,MAX_MAG,MULTIPLIER,OFFSET
MB,0,10,1.0,0.2
MS,0,10,0.95,0.4
ML,0,10,1.05,-0.1
EOF

echo "1. Fetching Data from ISC (2012)"
"$ISC_EXE" \
    --min-lon=95.0 --max-lon=105.0 \
    --min-lat=-7.0 --max-lat=6.0 \
    --start-date="2012-01-01" --start-time="00:00:00" \
    --end-date="2012-12-31" --end-time="23:59:59" \
    --min-depth=0.0 --max-depth=300.0 \
    --min-mag=4.5 --max-mag=9.0 \
    --output "katalog_sumatera_processed.csv" \
    --raw-output "raw_isc.txt"

echo "2. Applying Conversions"
"$ISC_EXE" \
    --input-raw "raw_isc.txt" \
    --conversion-file "aturan_konversi.csv" \
    --output "katalog_sumatera_seragam.csv"

echo "3. Generating Statistics and Map"
"$ISC_EXE" \
    --input-raw "raw_isc.txt" \
    --plot-map "peta_sebaran.png" \
    --plot-stats "output_direktori_grafik"

echo "4. Generating Cross Section"
"$ISC_EXE" \
    --input-raw "raw_isc.txt" \
    --cs-start-lon=95.0 --cs-start-lat=5.0 \
    --cs-end-lon=105.0 --cs-end-lat=-6.0 \
    --cs-buffer-km=150.0 \
    --cs-out-csv "cross_section_data.csv" \
    --plot-cross-section "cross_section_plot.png" \
    --plot-map "peta_sebaran_cs.png" \
    --plot-cs-track

echo "5. Building PDF with Pandoc"
cp ../tutorial_cfs/custom_header.tex .
pandoc tutorial_isc.md -o SeisBox_ISC_Manual.pdf -V geometry:margin=1in --toc --syntax-highlighting=tango --include-in-header=custom_header.tex

echo "Done!"
