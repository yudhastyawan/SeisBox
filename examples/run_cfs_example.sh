#!/bin/bash

# ====================================================================
# SeisBox CFS Bash Test Script
# Ported from python_cfs/run_cfs_tests.py
# ====================================================================

# Stop on first error
set -e

# Get directories
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" &> /dev/null && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

# Input files
INP_FILE="$SCRIPT_DIR/sofifi-ff-2.inp"
BATCH_FILE="$SCRIPT_DIR/fault_input_cat_all_lonlat.csv"

# Locate the executable (handling both source tree and release zip layouts)
if [ -f "$PROJECT_DIR/SeisBox_CLI/seisbox_cfs" ]; then
    CFS_EXE="$PROJECT_DIR/SeisBox_CLI/seisbox_cfs"
elif [ -f "$PROJECT_DIR/target/release/seisbox_cfs" ]; then
    CFS_EXE="$PROJECT_DIR/target/release/seisbox_cfs"
else
    echo "Error: Executable seisbox_cfs tidak ditemukan."
    echo "Jika dari source code, pastikan Anda sudah menjalankan 'cargo build --release'."
    exit 1
fi

echo "=========================================================="
echo "Memulai Pengujian SeisBox CFS via Bash Script"
echo "=========================================================="

# -------------------------------------------------------------
# 0a. TEST KASUS: INP Generation
# -------------------------------------------------------------
echo -e "\n[TEST GENERATE INP]"
GEN_INP_FILE="$SCRIPT_DIR/generated_fault.inp"
"$CFS_EXE" --generate-inp "$GEN_INP_FILE" \
    --gen-strike 45.0 \
    --gen-dip 80.0 \
    --gen-rake 90.0 \
    --gen-lon 128.0 \
    --gen-lat -1.0 \
    --gen-mag 7.2 \
    --gen-fault-sense ss
echo "✅ INP File generated: $GEN_INP_FILE"

# -------------------------------------------------------------
# 0b. TEST KASUS: INP Append
# -------------------------------------------------------------
echo -e "\n[TEST APPEND INP]"
"$CFS_EXE" --append-inp "$GEN_INP_FILE" \
    --gen-strike 120.0 \
    --gen-dip 70.0 \
    --gen-rake -10.0 \
    --gen-lon 128.5 \
    --gen-lat -1.5 \
    --gen-mag 6.5 \
    --gen-fault-sense norm
echo "✅ INP File appended: $GEN_INP_FILE"

# -------------------------------------------------------------
# 0c. TEST KASUS: File Validation
# -------------------------------------------------------------
echo -e "\n[TEST VALIDATE]"
"$CFS_EXE" -i "$INP_FILE" --validate
echo "✅ INP validation completed."

# -------------------------------------------------------------
# 1. TEST KASUS: Grid Calculation dengan Override Parameter
# -------------------------------------------------------------
echo -e "\n[TEST GRID CALCULATION]"
GRID_OUT="$SCRIPT_DIR/grid_out_test.csv"
"$CFS_EXE" -i "$INP_FILE" \
    --use-source-mech \
    --grid-lon-inc 0.05 \
    --grid-lat-inc 0.05 \
    --fric 0.6 \
    --depth 10.0 \
    -o "$GRID_OUT"

GRID_OUT_10="$SCRIPT_DIR/grid_out_test_10.csv"
if [ -f "$GRID_OUT_10" ]; then
    echo "Plotting Grid..."
    "$CFS_EXE" --plot-csv "$GRID_OUT_10" \
        --plot-out "$SCRIPT_DIR/grid_plot_10.png" \
        --plot-aspect-equal \
        --plot-contourf \
        --plot-inp "$INP_FILE" \
        --plot-fault-color black \
        --plot-fault-width 6 \
        --plot-fault-style dashed \
        --plot-cs-track \
        --plot-cs-track-color red \
        --plot-cs-track-style dashed \
        --plot-cs-track-width 4 \
        --cs-start-lon 127.0 \
        --cs-finish-lon 130.0 \
        --cs-start-lat -2.0 \
        --cs-finish-lat 1.0
fi

# -------------------------------------------------------------
# 2. TEST KASUS: Batch Calculation dari Katalog Seismisitas
# -------------------------------------------------------------
echo -e "\n[TEST BATCH CALCULATION]"
BATCH_OUT="$SCRIPT_DIR/batch_out_test.csv"
"$CFS_EXE" -i "$INP_FILE" \
    -b "$BATCH_FILE" \
    -o "$BATCH_OUT"

if [ -f "$BATCH_OUT" ]; then
    echo "Plotting Batch..."
    "$CFS_EXE" --plot-csv "$BATCH_OUT" --plot-out "$SCRIPT_DIR/batch_plot.png" --plot-aspect-equal --plot-contourf
fi

# -------------------------------------------------------------
# 3. TEST KASUS: Optimal Fault Calculation (Analytical/Eigen)
# -------------------------------------------------------------
echo -e "\n[TEST OPTIMAL EIGEN CALCULATION]"
OPTIMAL_OUT="$SCRIPT_DIR/optimal_eigen_out_test.csv"
"$CFS_EXE" -i "$INP_FILE" \
    --optimal-eigen \
    --grid-lon-inc 0.05 \
    --grid-lat-inc 0.05 \
    --depth 10.0 \
    --depth-finish 15.0 \
    --depth-inc 5.0 \
    --max-depth \
    --tiff \
    -o "$OPTIMAL_OUT"

# Plotting optimal depth 10
if [ -f "${OPTIMAL_OUT%.csv}_10.csv" ]; then
    "$CFS_EXE" --plot-csv "${OPTIMAL_OUT%.csv}_10.csv" --plot-out "$SCRIPT_DIR/optimal_eigen_plot_10.png" --plot-aspect-equal --plot-contourf
fi

# Plotting optimal depth 15
if [ -f "${OPTIMAL_OUT%.csv}_15.csv" ]; then
    "$CFS_EXE" --plot-csv "${OPTIMAL_OUT%.csv}_15.csv" --plot-out "$SCRIPT_DIR/optimal_eigen_plot_15.png" --plot-aspect-equal --plot-contourf
fi

# Plotting optimal max depth
if [ -f "${OPTIMAL_OUT%.csv}_max.csv" ]; then
    "$CFS_EXE" --plot-csv "${OPTIMAL_OUT%.csv}_max.csv" --plot-out "$SCRIPT_DIR/optimal_eigen_plot_max.png" --plot-aspect-equal --plot-contourf
fi

# -------------------------------------------------------------
# 4. TEST KASUS: Cross Section Calculation
# -------------------------------------------------------------
echo -e "\n[TEST CROSS SECTION CALCULATION]"
CS_OUT="$SCRIPT_DIR/cross_section_out_test.csv"
"$CFS_EXE" -i "$INP_FILE" \
    --cross-section \
    --use-source-mech \
    --fric 0.6 \
    --cs-start-lon 127.0 \
    --cs-finish-lon 130.0 \
    --cs-start-lat -2.0 \
    --cs-finish-lat 1.0 \
    --cs-dist-inc 10.0 \
    --depth 0.0 \
    --depth-finish 40.0 \
    --depth-inc 5.0 \
    -o "$CS_OUT"

if [ -f "$CS_OUT" ]; then
    echo "Plotting Cross Section..."
    "$CFS_EXE" --plot-csv "$CS_OUT" \
        --plot-out "$SCRIPT_DIR/cross_section_plot.png" \
        --plot-aspect-equal \
        --plot-contourf \
        --plot-inp "$INP_FILE" \
        --plot-fault-color black \
        --plot-fault-width 6 \
        --plot-fault-style dashed \
        --cs-start-lon 127.0 \
        --cs-finish-lon 130.0 \
        --cs-start-lat -2.0 \
        --cs-finish-lat 1.0
fi

echo -e "\n✅ Seluruh pengujian berhasil diselesaikan!"
