/// Represents a geographic coordinate in decimal degrees.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GeoPoint {
    pub lat: f64,
    pub lon: f64,
}

impl GeoPoint {
    pub fn new(lat: f64, lon: f64) -> Self {
        Self { lat, lon }
    }
}

/// Bounding box for map selection.
#[derive(Debug, Clone, Copy, Default)]
pub struct BoundingBox {
    pub bot_lat: f64,
    pub top_lat: f64,
    pub left_lon: f64,
    pub right_lon: f64,
}

impl BoundingBox {
    pub fn is_valid(&self) -> bool {
        self.bot_lat < self.top_lat && self.left_lon < self.right_lon
    }
}

/// Cross-section definition from Point A to Point B.
#[derive(Debug, Clone, Copy)]
pub struct CrossSectionLine {
    pub point_a: GeoPoint,
    pub point_b: GeoPoint,
    pub buffer_km: f64,
}

/// Earth's radius in kilometers
const EARTH_RADIUS_KM: f64 = 6371.0;

/// Calculate the Haversine distance between two points in km.
pub fn haversine_distance(p1: &GeoPoint, p2: &GeoPoint) -> f64 {
    let d_lat = (p2.lat - p1.lat).to_radians();
    let d_lon = (p2.lon - p1.lon).to_radians();
    
    let lat1 = p1.lat.to_radians();
    let lat2 = p2.lat.to_radians();
    
    let a = (d_lat / 2.0).sin().powi(2) +
            (d_lon / 2.0).sin().powi(2) * lat1.cos() * lat2.cos();
    
    let c = 2.0 * a.sqrt().asin();
    EARTH_RADIUS_KM * c
}

/// Calculate the initial bearing from p1 to p2 in radians.
pub fn initial_bearing(p1: &GeoPoint, p2: &GeoPoint) -> f64 {
    let lat1 = p1.lat.to_radians();
    let lat2 = p2.lat.to_radians();
    let d_lon = (p2.lon - p1.lon).to_radians();
    
    let y = d_lon.sin() * lat2.cos();
    let x = lat1.cos() * lat2.sin() - lat1.sin() * lat2.cos() * d_lon.cos();
    
    y.atan2(x)
}

/// Compute the cross-track (perpendicular) and along-track (parallel) distances of point P 
/// relative to the line from A to B.
/// 
/// Returns `(along_track_km, cross_track_km)`.
pub fn cross_section_projection(a: &GeoPoint, b: &GeoPoint, p: &GeoPoint) -> (f64, f64) {
    let dist_ap = haversine_distance(a, p);
    if dist_ap == 0.0 {
        return (0.0, 0.0);
    }
    
    let bearing_ab = initial_bearing(a, b);
    let bearing_ap = initial_bearing(a, p);
    
    // Angle difference
    let theta = bearing_ap - bearing_ab;
    
    // Using simple planar approximation for local cross sections:
    let cross_track = (dist_ap * theta.sin()).abs();
    let along_track = dist_ap * theta.cos();
    
    (along_track, cross_track)
}
