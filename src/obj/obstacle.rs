use super::*;

impl Obstacle {
    pub fn new(location: Location, radius: f32, height: f32) -> Self {
        Obstacle {
            location: location,
            radius: radius,
            height: height,
        }
    }
    pub fn from_degrees(lon: f64, lat: f64, radius: f32, height: f32) -> Self {
        Obstacle::new(Location::from_degrees(lon, lat, height), radius, height)
    }

    pub fn from_radians(lon: f64, lat: f64, radius: f32, height: f32) -> Self {
        Obstacle::new(Location::from_radians(lon, lat, height), radius, height)
    }
}
