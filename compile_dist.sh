#!/bin/bash

# Exit on any error
set -e

echo "====================================="
echo " Building SeisBox for MacOS (Release) "
echo "====================================="

# Compile in release mode
cargo build --release

# Create distribution folder
DIST_DIR="dist"
APP_NAME="SeisBox.app"
APP_DIR="$DIST_DIR/$APP_NAME/Contents/MacOS"
RES_DIR="$DIST_DIR/$APP_NAME/Contents/Resources"

echo "Creating distribution folder at $DIST_DIR..."
rm -rf "$DIST_DIR"
mkdir -p "$APP_DIR"
mkdir -p "$RES_DIR"

# Copy binary to distribution folder
echo "Copying binary..."
cp target/release/seisbox "$APP_DIR/SeisBox"

# Copy libexec to distribution folder
if [ -d "src/libexec" ]; then
    echo "Copying libexec utilities..."
    cp -r "src/libexec" "$APP_DIR/"
fi

# Copy icon to Resources folder
if [ -f "assets/seisbox.icns" ]; then
    echo "Copying application icon..."
    cp assets/seisbox.icns "$RES_DIR/"
fi

# Ensure binary is executable
chmod +x "$APP_DIR/SeisBox"

# Create a basic Info.plist for MacOS
cat > "$DIST_DIR/$APP_NAME/Contents/Info.plist" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>SeisBox</string>
    <key>CFBundleIdentifier</key>
    <string>com.seisbox.app</string>
    <key>CFBundleName</key>
    <string>SeisBox</string>
    <key>CFBundleIconFile</key>
    <string>seisbox.icns</string>
    <key>CFBundleVersion</key>
    <string>1.0.0</string>
    <key>CFBundleShortVersionString</key>
    <string>1.0.0</string>
    <key>LSMinimumSystemVersion</key>
    <string>10.11</string>
</dict>
</plist>
EOF

echo "Applying ad-hoc code signature for Apple Silicon compatibility..."
codesign --force --deep --sign - "$DIST_DIR/$APP_NAME"

echo "Creating zip distribution..."
cd "$DIST_DIR"
# Remove existing zip if any
rm -f "SeisBox_macOS.zip"

# Create a Fix_App.command script to help users remove the quarantine flag
cat > "Fix_App_First.command" << 'EOF'
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
EOF
chmod +x "Fix_App_First.command"

# Copy examples folder if it exists in the root directory
if [ -d "../examples" ]; then
    echo "Including examples folder in distribution..."
    cp -r "../examples" ./
    zip -r -q "SeisBox_macOS.zip" "$APP_NAME" "Fix_App_First.command" "examples"
else
    zip -r -q "SeisBox_macOS.zip" "$APP_NAME" "Fix_App_First.command"
fi
cd ..
echo "Zip file created at $DIST_DIR/SeisBox_macOS.zip"

echo "Done! The application is ready at $DIST_DIR/$APP_NAME"
echo "You can distribute the $DIST_DIR/SeisBox_macOS.zip file."

# Option to run it immediately
read -p "Do you want to run SeisBox now? (y/n) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]
then
    open "$DIST_DIR/$APP_NAME"
fi
