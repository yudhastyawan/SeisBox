use std::fs::File;
use std::io::Write;
use std::path::Path;
use tiff::encoder::{TiffEncoder, colortype};
use tiff::tags::Tag;

pub fn export_grid_to_tiff(
    path: &Path,
    data: &[f32],
    width: u32,
    height: u32,
    x_min: f64,
    x_max: f64,
    y_min: f64,
    y_max: f64,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut file = File::create(path)?;
    let mut encoder = TiffEncoder::new(&mut file)?;
    
    // Write the actual TIFF f32 image
    let mut image = encoder.new_image::<colortype::Gray32Float>(width, height)?;
    
    let pixel_width = if width > 1 { (x_max - x_min) / (width as f64 - 1.0) } else { 1.0 };
    let pixel_height = if height > 1 { (y_max - y_min) / (height as f64 - 1.0) } else { 1.0 };
    
    // ModelPixelScaleTag (33550) - MUST be positive values
    image.encoder().write_tag(Tag::Unknown(33550), &[pixel_width, pixel_height, 0.0][..])?;
    
    let tie_x = x_min - (pixel_width / 2.0);
    let tie_y = y_max + (pixel_height / 2.0);
    
    // ModelTiepointTag (33922) - Maps pixel top-left corner (0,0) to coordinate (tie_x, tie_y)
    image.encoder().write_tag(Tag::Unknown(33922), &[0.0, 0.0, 0.0, tie_x, tie_y, 0.0][..])?;
    
    // GeoKeyDirectoryTag (34735) - EPSG:4326 (WGS 84)
    let geokeys: [u16; 16] = [
        1, 1, 0, 3, // Header: KeyDirectoryVersion, KeyRevision, MinorRevision, NumberOfKeys
        1024, 0, 1, 2, // GTModelTypeGeoKey: 2 (Geographic 2D)
        1025, 0, 1, 1, // GTRasterTypeGeoKey: 1 (RasterPixelIsArea)
        2048, 0, 1, 4326 // GeographicTypeGeoKey: 4326 (WGS 84)
    ];
    image.encoder().write_tag(Tag::Unknown(34735), &geokeys[..])?;
    
    image.write_data(data)?;
    
    // Create the world file (.tfw)
    let path_str = path.to_string_lossy().to_string();
    let tfw_path = if path_str.to_lowercase().ends_with(".tif") {
        format!("{}.tfw", &path_str[..path_str.len() - 4])
    } else if path_str.to_lowercase().ends_with(".tiff") {
        format!("{}.tfw", &path_str[..path_str.len() - 5])
    } else {
        format!("{}.tfw", path_str)
    };
    
    let mut tfw_file = File::create(tfw_path)?;
    
    writeln!(tfw_file, "{:.10}", pixel_width)?;
    writeln!(tfw_file, "0.0")?;
    writeln!(tfw_file, "0.0")?;
    writeln!(tfw_file, "{:.10}", -pixel_height)?;
    writeln!(tfw_file, "{:.10}", x_min)?;
    writeln!(tfw_file, "{:.10}", y_max)?;
    
    Ok(())
}
