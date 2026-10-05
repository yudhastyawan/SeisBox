#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" &> /dev/null && pwd)"
PROJECT_DIR="$(dirname "$(dirname "$SCRIPT_DIR")")"
CFS_EXE="$PROJECT_DIR/target/release/seisbox_cfs"

echo "1. Generating Input File (Event 1 - Flores Sea M7.76)"
INP_FILE="$SCRIPT_DIR/tutorial_faults.inp"

"$CFS_EXE" --generate-inp "$INP_FILE" \
    --gen-strike 269.0 \
    --gen-dip 58.0 \
    --gen-rake 90.0 \
    --gen-lon 121.348 \
    --gen-lat -8.351 \
    --gen-depth 23.5 \
    --gen-mag 7.76 \
    --gen-fault-sense rev

echo "2. Appending Input File (Event 2 - Lombok M6.94)"
"$CFS_EXE" --append-inp "$INP_FILE" \
    --gen-strike 280.0 \
    --gen-dip 60.0 \
    --gen-rake 98.0 \
    --gen-lon 116.627 \
    --gen-lat -8.319 \
    --gen-depth 30.5 \
    --gen-mag 6.94 \
    --gen-fault-sense rev

echo "3. Validating INP File"
"$CFS_EXE" -i "$INP_FILE" --validate

# Wait, we need to explicitly center the grid around Event 1 or we need to give lon/lat coords for the grid instead of x/y?
# Currently SeisBox uses grid-x-start relative to the center of the faults bounding box.
# Let's just generate the grid as is, it might cover both. But if it's too big, let's see how it goes.
echo "4. Grid Calculation (Focused on Event 1 Region)"
GRID_OUT="$SCRIPT_DIR/grid_cfs.csv"
"$CFS_EXE" -i "$INP_FILE" \
    --use-source-mech \
    --grid-lon-inc 0.05 \
    --grid-lat-inc 0.05 \
    --fric 0.4 \
    --depth 20.0 \
    -o "$GRID_OUT"

echo "5. Plotting Grid"
"$CFS_EXE" --plot-csv "${GRID_OUT%.csv}_20.csv" \
    --plot-out "$SCRIPT_DIR/grid_plot.png" \
    --plot-aspect-equal \
    --plot-contourf \
    --plot-inp "$INP_FILE" \
    --plot-fault-color black \
    --plot-fault-width 4 \
    --plot-cs-track \
    --plot-cs-track-color blue \
    --plot-cs-track-width 4 \
    --plot-cs-track-style dashed \
    --cs-start-lon 120.0 \
    --cs-finish-lon 122.5 \
    --cs-start-lat -9.5 \
    --cs-finish-lat -7.0

echo "6. Cross-Section Calculation"
CS_OUT="$SCRIPT_DIR/cross_section.csv"
"$CFS_EXE" -i "$INP_FILE" \
    --cross-section \
    --use-source-mech \
    --fric 0.4 \
    --cs-start-lon 120.0 \
    --cs-finish-lon 122.5 \
    --cs-start-lat -9.5 \
    --cs-finish-lat -7.0 \
    --cs-dist-inc 5.0 \
    --depth 0.0 \
    --depth-finish 60.0 \
    --depth-inc 2.0 \
    -o "$CS_OUT"

echo "7. Plotting Cross-Section"
"$CFS_EXE" --plot-csv "$CS_OUT" \
    --plot-out "$SCRIPT_DIR/cross_section_plot.png" \
    --plot-aspect-equal \
    --plot-contourf \
    --plot-inp "$INP_FILE" \
    --plot-fault-color black \
    --plot-fault-width 4 \
    --cs-start-lon 120.0 \
    --cs-finish-lon 122.5 \
    --cs-start-lat -9.5 \
    --cs-finish-lat -7.0

echo "Tutorial generation complete."
