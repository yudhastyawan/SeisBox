#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" &> /dev/null && pwd)"
PROJECT_DIR="$(dirname "$(dirname "$SCRIPT_DIR")")"
CFS_EXE="$PROJECT_DIR/target/release/seisbox_cfs"

# Change to the SCRIPT_DIR so that the commands mimic the "local" execution style we want in the tutorial
cd "$SCRIPT_DIR"

INP_FILE="tutorial_faults.inp"
ASSETS_DIR="assets"
mkdir -p "$ASSETS_DIR"

echo "1. Generating Input File (Event 1 - NP2)"
"$CFS_EXE" --generate-inp "$INP_FILE" \
    --gen-strike 90.0 \
    --gen-dip 32.0 \
    --gen-rake 90.0 \
    --gen-lon 121.348 \
    --gen-lat -8.351 \
    --gen-depth 23.5 \
    --gen-mag 7.76 \
    --gen-fault-sense rev

echo "2. Validating Input File"
"$CFS_EXE" -i "$INP_FILE" --validate

echo "3. Basic Grid Calculation"
GRID_OUT="grid_cfs_20.csv"
"$CFS_EXE" -i "$INP_FILE" \
    --use-source-mech \
    --grid-lon-inc 0.05 \
    --grid-lat-inc 0.05 \
    --fric 0.4 \
    --depth 20.0 \
    -o "grid_cfs.csv"

echo "4. Generating Main Plots..."
"$CFS_EXE" --plot-csv "$GRID_OUT" --plot-out "$ASSETS_DIR/grid_step1_basic.png" --plot-aspect-equal
"$CFS_EXE" --plot-csv "$GRID_OUT" --plot-out "$ASSETS_DIR/grid_step2_contourf.png" --plot-aspect-equal --plot-contourf
"$CFS_EXE" --plot-csv "$GRID_OUT" --plot-out "$ASSETS_DIR/grid_step3_faults.png" --plot-aspect-equal --plot-contourf --plot-inp "$INP_FILE"
"$CFS_EXE" --plot-csv "$GRID_OUT" --plot-out "$ASSETS_DIR/grid_step4_styled_faults.png" --plot-aspect-equal --plot-contourf --plot-inp "$INP_FILE" --plot-fault-color magenta --plot-fault-width 5 --plot-fault-style dashed
"$CFS_EXE" --plot-csv "$GRID_OUT" --plot-out "$ASSETS_DIR/grid_step5_cs_track.png" --plot-aspect-equal --plot-contourf --plot-inp "$INP_FILE" --plot-fault-color black --plot-fault-width 3 --plot-cs-track --plot-cs-track-color red --plot-cs-track-style dashed --plot-cs-track-width 4 --cs-start-lon 120.5 --cs-finish-lon 122.2 --cs-start-lat -9.2 --cs-finish-lat -7.5

echo "5. Cross-Section Calculation"
CS_OUT="cross_section.csv"
"$CFS_EXE" -i "$INP_FILE" \
    --cross-section \
    --use-source-mech \
    --fric 0.4 \
    --cs-start-lon 120.5 \
    --cs-finish-lon 122.2 \
    --cs-start-lat -9.2 \
    --cs-finish-lat -7.5 \
    --cs-dist-inc 5.0 \
    --depth 0.0 \
    --depth-finish 60.0 \
    --depth-inc 2.0 \
    -o "$CS_OUT"

"$CFS_EXE" --plot-csv "$CS_OUT" --plot-out "$ASSETS_DIR/cs_final.png" --plot-aspect-equal --plot-contourf --plot-inp "$INP_FILE" --plot-fault-color black --plot-fault-width 4 --cs-start-lon 120.5 --cs-finish-lon 122.2 --cs-start-lat -9.2 --cs-finish-lat -7.5

echo "6. Advanced Grid: Expanded Boundaries"
"$CFS_EXE" -i "$INP_FILE" \
    --use-source-mech \
    --grid-lon-inc 0.05 \
    --grid-lat-inc 0.05 \
    --grid-start-x -200.0 \
    --grid-finish-x 200.0 \
    --grid-start-y -200.0 \
    --grid-finish-y 200.0 \
    --fric 0.4 \
    --depth 20.0 \
    -o "grid_expanded.csv"

"$CFS_EXE" --plot-csv "grid_expanded_20.csv" --plot-out "$ASSETS_DIR/grid_expanded_plot.png" --plot-aspect-equal --plot-contourf --plot-inp "$INP_FILE" --plot-fault-color black --plot-fault-width 3

echo "7. Advanced Grid: Custom Receiver Mechanism (Strike-Slip)"
"$CFS_EXE" -i "$INP_FILE" \
    --strike 90.0 \
    --dip 90.0 \
    --rake 0.0 \
    --grid-lon-inc 0.05 \
    --grid-lat-inc 0.05 \
    --fric 0.4 \
    --depth 20.0 \
    -o "grid_custom.csv"

"$CFS_EXE" --plot-csv "grid_custom_20.csv" --plot-out "$ASSETS_DIR/grid_custom_plot.png" --plot-aspect-equal --plot-contourf --plot-inp "$INP_FILE" --plot-fault-color black --plot-fault-width 3

echo "7b. Batch Mode Calculation (USGS Aftershocks)"
echo "lon,lat,z,strike,dip,rake" > receivers.csv
tail -n +2 events.csv | \
    awk -F',' '{
        if($3 >= 120.0 && $3 <= 123.0 && $2 >= -9.5 && $2 <= -7.0) 
            print $3","$2","$4",90,32,90"
    }' >> receivers.csv

"$CFS_EXE" -i "$INP_FILE" -b "receivers.csv" \
    --fric 0.4 --poisson 0.25 --young 800000.0 \
    -o "batch_output.csv"

"$CFS_EXE" --plot-csv "batch_output.csv" --plot-out "$ASSETS_DIR/batch_plot.png" \
    --plot-aspect-equal --plot-inp "$INP_FILE" \
    --plot-vmin -1.5 --plot-vmax 1.5

echo "8. Appending Event 2 (Flores 2021 M7.31)"
cp tutorial_faults.inp tutorial_faults_2.inp
"$CFS_EXE" --append-inp "tutorial_faults_2.inp" \
    --gen-strike 290.0 \
    --gen-dip 89.0 \
    --gen-rake 177.0 \
    --gen-lon 122.227 \
    --gen-lat -7.603 \
    --gen-depth 17.5 \
    --gen-mag 7.31 \
    --gen-fault-sense ss

"$CFS_EXE" -i "tutorial_faults_2.inp" \
    --use-source-mech \
    --grid-start-lon 120.5 \
    --grid-finish-lon 123.5 \
    --grid-start-lat -9.5 \
    --grid-finish-lat -6.5 \
    --grid-lon-inc 0.05 \
    --grid-lat-inc 0.05 \
    --fric 0.4 \
    --depth 20.0 \
    -o "grid_cfs_2.csv"

"$CFS_EXE" --plot-csv "grid_cfs_2_20.csv" --plot-out "$ASSETS_DIR/grid_multi_event.png" --plot-aspect-equal --plot-contourf --plot-inp "tutorial_faults_2.inp" --plot-fault-color black --plot-fault-width 3

echo "Done generating assets!"
