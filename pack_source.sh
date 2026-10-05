#!/bin/bash

# Nama file output
ZIP_NAME="SeisBoxSource.zip"

echo "Mempersiapkan kompresi source code ke $ZIP_NAME..."

# Hapus file zip lama jika sudah ada
if [ -f "$ZIP_NAME" ]; then
    rm "$ZIP_NAME"
    echo "File $ZIP_NAME lama telah dihapus."
fi

# Jalankan kompresi menggunakan zip
# -r : recursive (termasuk isi folder)
# -x : mengecualikan file/folder tertentu seperti target/ dan file sistem
zip -r "$ZIP_NAME" \
    Cargo.toml \
    Cargo.lock \
    apps \
    core \
    assets \
    compile_dist.bat \
    compile_dist.sh \
    build_msi.bat \
    build_msi_v5.bat \
    README.md \
    LICENSE \
    -x "*/target/*" \
    -x "*/.DS_Store" \
    -x "*/.git/*" \
    -x "*/__pycache__/*"

echo "================================================="
echo "Selesai! Source code berhasil dikompres ke dalam: $ZIP_NAME"
echo "================================================="
