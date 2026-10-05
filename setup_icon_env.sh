#!/bin/bash
# =============================================================================
# setup_icon_env.sh
# Membuat conda environment lokal di folder ini (.conda_env/) dan
# menginstal tools untuk konversi SVG → PNG → ICNS
#
# Cara pakai:
#   chmod +x setup_icon_env.sh
#   ./setup_icon_env.sh
# =============================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ENV_DIR="$SCRIPT_DIR/.conda_env"
SVG_INPUT="$SCRIPT_DIR/assets/seisbox_icon.svg"
ICONSET_DIR="$SCRIPT_DIR/assets/SeisBox.iconset"
OUTPUT_ICNS="$SCRIPT_DIR/assets/seisbox.icns"

echo ""
echo "╔══════════════════════════════════════════════╗"
echo "║      SeisBox Icon Environment Setup          ║"
echo "╚══════════════════════════════════════════════╝"
echo ""

# ── STEP 1: Buat conda env lokal ──────────────────────────────────────────────
if [ -d "$ENV_DIR" ]; then
    echo "✔ Conda environment sudah ada di: $ENV_DIR"
else
    echo "► Membuat conda environment di: $ENV_DIR"
    conda create --prefix "$ENV_DIR" python=3.11 -y
    echo "✔ Environment berhasil dibuat."
fi

# ── STEP 2: Instal dependensi ──────────────────────────────────────────────────
echo ""
echo "► Menginstal librsvg (rsvg-convert) dan cairosvg ..."
conda install --prefix "$ENV_DIR" -c conda-forge librsvg cairosvg -y

echo ""
echo "✔ Semua package berhasil diinstal."

# ── STEP 3: Konversi SVG → PNG (berbagai ukuran untuk iconset) ────────────────
echo ""
echo "► Membuat PNG dari SVG untuk semua ukuran ikon macOS ..."

mkdir -p "$ICONSET_DIR"

RSVG="$ENV_DIR/bin/rsvg-convert"

# Ukuran yang dibutuhkan macOS iconset
declare -a SIZES=("16" "32" "64" "128" "256" "512" "1024")

for SIZE in "${SIZES[@]}"; do
    echo "  → ${SIZE}×${SIZE} px"
    "$RSVG" -w "$SIZE" -h "$SIZE" "$SVG_INPUT" \
        -o "$ICONSET_DIR/icon_${SIZE}x${SIZE}.png"
done

# macOS membutuhkan pasangan @1x dan @2x
# Contoh: icon_16x16.png + icon_16x16@2x.png (= 32px)
cp "$ICONSET_DIR/icon_32x32.png"   "$ICONSET_DIR/icon_16x16@2x.png"
cp "$ICONSET_DIR/icon_64x64.png"   "$ICONSET_DIR/icon_32x32@2x.png"
cp "$ICONSET_DIR/icon_256x256.png" "$ICONSET_DIR/icon_128x128@2x.png"
cp "$ICONSET_DIR/icon_512x512.png" "$ICONSET_DIR/icon_256x256@2x.png"
cp "$ICONSET_DIR/icon_1024x1024.png" "$ICONSET_DIR/icon_512x512@2x.png" 2>/dev/null || \
    "$RSVG" -w 1024 -h 1024 "$SVG_INPUT" -o "$ICONSET_DIR/icon_512x512@2x.png"

echo "✔ Semua PNG berhasil dibuat di: $ICONSET_DIR"

# ── STEP 4: Konversi iconset → ICNS (pakai iconutil bawaan macOS) ──────────────
echo ""
echo "► Membuat file .icns dengan iconutil ..."
iconutil -c icns "$ICONSET_DIR" -o "$OUTPUT_ICNS"
echo "✔ File ICNS berhasil dibuat: $OUTPUT_ICNS"

# ── STEP 5: Salin ke dist app bundle (opsional) ────────────────────────────────
BUNDLE_RESOURCES="$SCRIPT_DIR/dist/SeisBox.app/Contents/Resources"
if [ -d "$BUNDLE_RESOURCES" ]; then
    echo ""
    echo "► Menyalin .icns ke app bundle ..."
    cp "$OUTPUT_ICNS" "$BUNDLE_RESOURCES/seisbox.icns"
    echo "✔ ICNS disalin ke: $BUNDLE_RESOURCES"
fi

echo ""
echo "╔══════════════════════════════════════════════╗"
echo "║  Selesai! File yang dihasilkan:              ║"
echo "║  • assets/SeisBox.iconset/ (semua PNG)       ║"
echo "║  • assets/seisbox.icns    (icon macOS)       ║"
echo "╚══════════════════════════════════════════════╝"
echo ""
