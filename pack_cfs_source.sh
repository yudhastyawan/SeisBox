#!/bin/bash

# Nama file output
ZIP_NAME="SeisBoxCFSSource.zip"

echo "Mempersiapkan kompresi source code CFS ke $ZIP_NAME..."

# Hapus file zip lama jika sudah ada
if [ -f "$ZIP_NAME" ]; then
    rm "$ZIP_NAME"
    echo "File $ZIP_NAME lama telah dihapus."
fi

# Buat direktori sementara untuk merakit paket source
TMP_DIR="tmp_cfs_source"
rm -rf "$TMP_DIR"
mkdir -p "$TMP_DIR"

# Salin direktori dan fail yang diperlukan
cp Cargo.toml Cargo.lock README.md LICENSE compile_cfs_dist.bat compile_cfs_dist.sh "$TMP_DIR/"
mkdir -p "$TMP_DIR/apps" "$TMP_DIR/docs"
cp -r apps/seisbox_cfs "$TMP_DIR/apps/"
cp -r core "$TMP_DIR/"
cp -r docs/tutorial_cfs "$TMP_DIR/docs/"

# Hapus workspace members yang tidak relevan dari Cargo.toml agar tidak error saat di-build
if [[ "$OSTYPE" == "darwin"* ]]; then
    sed -i '' -e '/"apps\/seisbox_launcher"/d' -e '/"apps\/seisbox_picker"/d' -e '/"apps\/seisbox_stats"/d' -e '/"apps\/seisbox_hvsr"/d' -e '/"apps\/seisbox_inversion"/d' -e '/"apps\/seisbox_interp"/d' -e '/"apps\/seisbox_fdsn"/d' -e '/"apps\/seisbox_isc"/d' "$TMP_DIR/Cargo.toml"
else
    sed -i -e '/"apps\/seisbox_launcher"/d' -e '/"apps\/seisbox_picker"/d' -e '/"apps\/seisbox_stats"/d' -e '/"apps\/seisbox_hvsr"/d' -e '/"apps\/seisbox_inversion"/d' -e '/"apps\/seisbox_interp"/d' -e '/"apps\/seisbox_fdsn"/d' -e '/"apps\/seisbox_isc"/d' "$TMP_DIR/Cargo.toml"
fi

echo "Membuat file zip..."
cd "$TMP_DIR"
zip -r -q "../$ZIP_NAME" . -x "*/target/*" -x "*/.DS_Store" -x "*/.git/*" -x "*/__pycache__/*"
cd ..

# Bersihkan direktori sementara
rm -rf "$TMP_DIR"

echo "================================================="
echo "Selesai! Source code CFS berhasil dikompres ke dalam: $ZIP_NAME"
echo "================================================="
