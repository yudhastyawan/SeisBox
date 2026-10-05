#!/bin/bash

# Exit on any error
set -e

echo "==================================================="
echo " Building SeisBox CFS for MacOS (Release Mode)     "
echo "==================================================="

# Compile only the CFS package
cargo build --release --package seisbox_cfs

# Create distribution folder
DIST_DIR="dist_mac_cfs"

echo "Creating distribution folder at $DIST_DIR..."
rm -rf "$DIST_DIR"
mkdir -p "$DIST_DIR"

# Extract CLI binaries to a dedicated folder for easy access in terminal
CLI_DIR="$DIST_DIR/SeisBox_CFS_CLI"
echo "Creating dedicated CLI folder..."
mkdir -p "$CLI_DIR"

# Copy the binary
cp "target/release/seisbox_cfs" "$CLI_DIR/"
chmod +x "$CLI_DIR/"*

ZIP_ARGS=()
ZIP_ARGS+=("SeisBox_CFS_CLI")

# Remove existing zip if any
rm -f "SeisBox_CFS_macOS.zip"

# Copy tutorial folder
echo "Including docs/tutorial_cfs in distribution..."
mkdir -p "$DIST_DIR/docs"
cp -r "docs/tutorial_cfs" "$DIST_DIR/docs/"
ZIP_ARGS+=("docs")

echo "Creating zip distribution..."
cd "$DIST_DIR"
zip -r -q "../SeisBox_CFS_macOS.zip" "${ZIP_ARGS[@]}"
cd ..
echo "Zip file created at SeisBox_CFS_macOS.zip"

echo "Done! The application is ready at $DIST_DIR/"
