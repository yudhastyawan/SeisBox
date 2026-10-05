#!/bin/bash

# Script to easily change version numbers across SeisBox files
# Author: Antigravity

if [ "$#" -ne 2 ]; then
    echo "Usage: ./set_version.sh <OLD_VERSION> <NEW_VERSION>"
    echo "Example: ./set_version.sh 0.1.0 0.1.1"
    exit 1
fi

OLD_VER=$1
NEW_VER=$2

echo "Replacing version $OLD_VER with $NEW_VER in SeisBox files..."

# 1. Update Cargo.toml files (Rust workspace and packages)
# Using find to get all Cargo.toml files except those in target/
find . -name "Cargo.toml" -not -path "*/target/*" -not -path "*/releases/*" -exec sed -i '' "s/version = \"$OLD_VER\"/version = \"$NEW_VER\"/g" {} +

# 2. Update Windows Batch Installers
if [ -f "build_msi.bat" ]; then
    sed -i '' "s/set PRODUCT_VERSION=$OLD_VER/set PRODUCT_VERSION=$NEW_VER/g" build_msi.bat
fi
if [ -f "build_msi_v5.bat" ]; then
    sed -i '' "s/set PRODUCT_VERSION=$OLD_VER/set PRODUCT_VERSION=$NEW_VER/g" build_msi_v5.bat
fi

# 3. Update macOS compilation script (Info.plist contents)
if [ -f "compile_dist.sh" ]; then
    sed -i '' "s/<string>$OLD_VER<\/string>/<string>$NEW_VER<\/string>/g" compile_dist.sh
fi

# 4. Update README.md badge
if [ -f "README.md" ]; then
    sed -i '' "s/badge\/version-$OLD_VER-green/badge\/version-$NEW_VER-green/g" README.md
fi

# 5. Update Documentation HTML
if [ -f "docs/index.html" ]; then
    sed -i '' "s/v$OLD_VER — Now Available/v$NEW_VER — Now Available/g" docs/index.html
fi

echo "Done!"
echo "Note: Make sure to run 'cargo build' or 'cargo check' to automatically update your Cargo.lock file with the new version."
