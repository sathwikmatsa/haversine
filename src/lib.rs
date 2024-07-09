use serde::{Deserialize, Serialize};

pub const EARTH_RADIUS: f64 = 6372.8f64;
pub const X_LOW: f64 = -180f64;
pub const X_HIGH: f64 = 180f64;
pub const Y_LOW: f64 = -90f64;
pub const Y_HIGH: f64 = 90f64;

#[derive(Serialize, Deserialize, Debug)]
pub struct HaversineDataPoint {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

#[derive(Serialize, Deserialize)]
pub struct HaversineData {
    pub pairs: Vec<HaversineDataPoint>,
}

// Reference: https://github.com/cmuratori/computer_enhance/blob/a6e9cb2a7b57e450ba2e7b75d0fd3e36ffa72d7d/perfaware/part2/listing_0065_haversine_formula.cpp
#[must_use] pub fn reference_haversine(point: &HaversineDataPoint, radius: f64) -> f64 {
    let lat1 = point.y0;
    let lat2 = point.y1;
    let lon1 = point.x0;
    let lon2 = point.x1;

    let d_lat = (lat2 - lat1).to_radians();
    let d_lon = (lon2 - lon1).to_radians();
    let lat1_rad = lat1.to_radians();
    let lat2_rad = lat2.to_radians();

    let a = (d_lat / 2.0).sin().powf(2.0)
        + lat1_rad.cos() * lat2_rad.cos() * ((d_lon / 2.0).sin().powf(2.0));

    let c = 2.0 * a.sqrt().asin();

    radius * c
}
