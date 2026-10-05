use std::path::Path;
use plotters::prelude::*;
use super::api::FdsnStation;

pub fn generate_spatial_map_viz<P: AsRef<Path>>(
    stations: &[FdsnStation],
    ref_lat: f64,
    ref_lon: f64,
    out_path: P,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut min_lon = ref_lon;
    let mut max_lon = ref_lon;
    let mut min_lat = ref_lat;
    let mut max_lat = ref_lat;

    for sta in stations {
        if sta.lon < min_lon { min_lon = sta.lon; }
        if sta.lon > max_lon { max_lon = sta.lon; }
        if sta.lat < min_lat { min_lat = sta.lat; }
        if sta.lat > max_lat { max_lat = sta.lat; }
    }

    let lon_margin = (max_lon - min_lon) * 0.1;
    let lat_margin = (max_lat - min_lat) * 0.1;
    let lon_margin = if lon_margin == 0.0 { 1.0 } else { lon_margin };
    let lat_margin = if lat_margin == 0.0 { 1.0 } else { lat_margin };

    min_lon -= lon_margin; max_lon += lon_margin;
    min_lat -= lat_margin; max_lat += lat_margin;

    let width = 1000;
    let height = 800;
    let root = BitMapBackend::new(&out_path, (width, height)).into_drawing_area();
    root.fill(&WHITE)?;

    let mut chart = ChartBuilder::on(&root)
        .caption(format!("Spatial Distribution ({} Stations)", stations.len()), ("sans-serif", 30).into_font())
        .margin(30)
        .x_label_area_size(50)
        .y_label_area_size(50)
        .build_cartesian_2d(min_lon..max_lon, min_lat..max_lat)?;

    chart.configure_mesh()
        .x_desc("Longitude")
        .y_desc("Latitude")
        .axis_desc_style(("sans-serif", 20).into_font())
        .draw()?;

    // Draw Reference Point
    chart.draw_series(std::iter::once(EmptyElement::at((0.0, 0.0))))?
        .label("Reference Point")
        .legend(|(x, y)| Cross::new((x, y), 8, RGBColor(220, 20, 60).filled()));
        
    chart.draw_series(std::iter::once(
        Cross::new((ref_lon, ref_lat), 10, RGBColor(220, 20, 60).filled().stroke_width(3))
    ))?;

    // Draw Stations
    chart.draw_series(std::iter::once(EmptyElement::at((0.0, 0.0))))?
        .label("Station")
        .legend(|(x, y)| TriangleMarker::new((x, y), 6, RGBColor(31, 120, 180).filled()));

    chart.draw_series(
        stations.iter().map(|sta| {
            TriangleMarker::new((sta.lon, sta.lat), 8, RGBColor(31, 120, 180).filled())
        })
    )?;

    chart.configure_series_labels()
        .background_style(WHITE.mix(0.8))
        .border_style(BLACK)
        .position(SeriesLabelPosition::UpperRight)
        .label_font(("sans-serif", 20))
        .draw()?;

    root.present()?;
    Ok(())
}
