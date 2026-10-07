#!/bin/bash

# Exit on any error
set -e

echo "==================================================="
echo " Building SeisBox Suite for MacOS (Release Mode)   "
echo "==================================================="

# Compile the entire workspace in release mode
cargo build --release

# Create distribution folder
DIST_DIR="dist"

echo "Creating distribution folder at $DIST_DIR..."
rm -rf "$DIST_DIR"
mkdir -p "$DIST_DIR"

create_app_bundle() {
    local BIN_NAME=$1
    local APP_DISPLAY_NAME=$2
    local TARGET_DIR="$DIST_DIR/$APP_DISPLAY_NAME.app"
    local APP_MAC="$TARGET_DIR/Contents/MacOS"
    local APP_RES="$TARGET_DIR/Contents/Resources"
    
    echo "Creating $APP_DISPLAY_NAME.app..."
    mkdir -p "$APP_MAC"
    mkdir -p "$APP_RES"
    
    cp "target/release/$BIN_NAME" "$APP_MAC/$APP_DISPLAY_NAME"
    chmod +x "$APP_MAC/$APP_DISPLAY_NAME"
    
    if [ -f "assets/seisbox.icns" ]; then
        cp assets/seisbox.icns "$APP_RES/"
    fi
    
    cat > "$TARGET_DIR/Contents/Info.plist" << PLIST_EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>$APP_DISPLAY_NAME</string>
    <key>CFBundleIdentifier</key>
    <string>com.seisbox.${BIN_NAME}</string>
    <key>CFBundleName</key>
    <string>$APP_DISPLAY_NAME</string>
    <key>CFBundleIconFile</key>
    <string>seisbox.icns</string>
    <key>CFBundleVersion</key>
    <string>0.1.2</string>
    <key>CFBundleShortVersionString</key>
    <string>0.1.2</string>
    <key>LSMinimumSystemVersion</key>
    <string>10.11</string>
</dict>
</plist>
PLIST_EOF

    codesign --force --deep --sign - "$TARGET_DIR"
    ZIP_ARGS+=("$APP_DISPLAY_NAME.app")
}

ZIP_ARGS=()
create_app_bundle "seisbox_launcher" "SeisBox"
create_app_bundle "seisbox_picker" "SeisBox Picker"
create_app_bundle "seisbox_stats" "SeisBox Stats"
create_app_bundle "seisbox_hvsr" "SeisBox HVSR"
create_app_bundle "seisbox_inversion" "SeisBox Inversion"
create_app_bundle "seisbox_cfs" "SeisBox CFS"
create_app_bundle "seisbox_interp" "SeisBox Interpolator"
create_app_bundle "seisbox_fdsn" "SeisBox FDSN"
create_app_bundle "seisbox_isc" "SeisBox ISC"

echo "Creating zip distribution..."
cd "$DIST_DIR"
# Extract CLI binaries to a dedicated folder for easy access in terminal
CLI_DIR="SeisBox_CLI"
echo "Creating dedicated CLI folder..."
mkdir -p "$CLI_DIR"
# Copy binaries that have CLI capabilities
cp "../target/release/seisbox_cfs" "$CLI_DIR/"
chmod +x "$CLI_DIR/"*
ZIP_ARGS+=("$CLI_DIR")

# Remove existing zip if any
rm -f "SeisBox_macOS.zip"

# Create a Fix_App.command script to help users remove the quarantine flag
cat > "Fix_App_First.command" << 'FIX_EOF'
#!/bin/bash
DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
echo "======================================"
echo "    Fixing SeisBox App Permissions    "
echo "======================================"
echo "Removing macOS quarantine attributes..."
xattr -cr "$DIR/SeisBox.app" 2>/dev/null
echo ""
echo "Done! You can now close this terminal and double-click SeisBox.app to open it."
echo ""
FIX_EOF
chmod +x "Fix_App_First.command"

# Copy dependency and asset folders if they exist in the root directory
FOLDERS=("examples" "docs" "assets")
ZIP_ARGS+=("Fix_App_First.command")

for FOLDER in "${FOLDERS[@]}"; do
    if [ -d "../$FOLDER" ]; then
        echo "Including $FOLDER in distribution..."
        cp -r "../$FOLDER" ./
        ZIP_ARGS+=("$FOLDER")
    fi
done

zip -r -q "SeisBox_macOS.zip" "${ZIP_ARGS[@]}"
cd ..
echo "Zip file created at $DIST_DIR/SeisBox_macOS.zip"

echo "Done! The applications are ready at $DIST_DIR/"
echo "You can distribute the $DIST_DIR/SeisBox_macOS.zip file."

# Option to run it immediately
read -p "Do you want to run SeisBox now? (y/n) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]
then
    open "$DIST_DIR/SeisBox.app"
fi
