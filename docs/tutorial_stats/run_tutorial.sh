#!/bin/bash
set -e

# Run from SeisPick root directory
ROOT_DIR=$(pwd)
TUTORIAL_DIR="docs/tutorial_stats"
cd $TUTORIAL_DIR

# echo "Downloading ISC data..."
# cargo run --release -p seisbox_isc --bin seisbox_isc -- --start-date 2014-01-01 --end-date 2015-01-01 --min-lat -6.0 --max-lat 6.0 --min-lon 95.0 --max-lon 105.0 --min-mag 3.0 --output sumatra_raw.csv > out_1_download.txt 2>&1
# echo "Download complete."

# echo "Filtering data..."
# cargo run --release -p seisbox_stats --bin seisbox_stats -- filter --input sumatra_raw.csv --slab-file ../../Slab/sum_slab2_dep_02.23.18.xyz --slab-buffer 20.0 --max-depth 300.0 --output sumatra_slab.csv > out_2_filter.txt 2>&1
# echo "Filtering complete."

# echo "Declustering data..."
# cargo run --release -p seisbox_stats --bin seisbox_stats -- decluster --input sumatra_slab.csv --method gardner-knopoff --window uhrhammer --output sumatra_declustered.csv > out_3_decluster.txt 2>&1
# echo "Declustering complete."

echo "Plotting FMD..."
cargo run --release -p seisbox_stats --bin seisbox_stats -- plot --input sumatra_declustered.csv --plot-type fmd --output fmd_plot.png > out_4_fmd.txt 2>&1
echo "Plotting FMD complete."

echo "Plotting Cumulative..."
cargo run --release -p seisbox_stats --bin seisbox_stats -- plot --input sumatra_declustered.csv --plot-type cumulative --output cum_plot.png > out_5_cum.txt 2>&1
echo "Plotting Cumulative complete."

echo "Gridding b-value..."
cargo run --release -p seisbox_stats --bin seisbox_stats -- grid --input sumatra_declustered.csv --method constant-r --radius-km 50.0 --min-events 5 --param bvalue --grid-inc 0.1 --slab-file ../../Slab/sum_slab2_dep_02.23.18.xyz --slab-buffer 20.0 --tiff --output bvalue_grid.csv > out_6_grid.txt 2>&1
echo "Gridding b-value complete."

echo "Plotting b-value Grid..."
cargo run --release -p seisbox_stats --bin seisbox_stats -- plot-grid --input bvalue_grid.csv --output bvalue_grid_plot.png --param b-value --point-radius 5 > out_10_plot_bgrid.txt 2>&1
echo "Plotting b-value Grid complete."

echo "Extracting Gutenberg-Richter Parameters..."
cargo run --release -p seisbox_stats --bin seisbox_stats -- bvalue --input sumatra_declustered.csv --method mle --bin-width 0.1 > out_7_bvalue.txt 2>&1
echo "Extracting GR complete."

echo "Fitting Modified Omori Law..."
cargo run --release -p seisbox_stats --bin seisbox_stats -- omori --input sumatra_raw.csv --mainshock-time "2014-01-25T05:14:20" --max-days 100.0 --min-mag 3.0 > out_8_omori.txt 2>&1
echo "Fitting Omori Law complete."

echo "Mapping Z-value (LTA/STA)..."
cargo run --release -p seisbox_stats --bin seisbox_stats -- grid --input sumatra_raw.csv --method constant-r --radius-km 50.0 --min-events 10 --param zvalue --z-tw1 "2014-01-01/2014-06-01" --z-tw2 "2014-06-02/2015-01-01" --grid-inc 0.1 --slab-file ../../Slab/sum_slab2_dep_02.23.18.xyz --slab-buffer 20.0 --tiff --output zvalue_grid.csv > out_9_zvalue.txt 2>&1
echo "Mapping Z-value complete."

echo "Plotting Z-value Grid..."
cargo run --release -p seisbox_stats --bin seisbox_stats -- plot-grid --input zvalue_grid.csv --output zvalue_grid_plot.png --param Z-value --point-radius 5 > out_11_plot_zgrid.txt 2>&1
echo "Plotting Z-value Grid complete."

echo "All steps completed."
