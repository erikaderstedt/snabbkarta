use super::sweref_to_wgs84::Wgs84;

#[link(name = "WMM", kind = "static")]
extern {
    fn todays_magnetic_declination(latitude: f64, longitude: f64, height_over_sea_level: f64) -> f64;
}

pub fn get_todays_magnetic_declination(position: &Wgs84, height_over_sea_level: f64) -> f64 {
    unsafe { todays_magnetic_declination(position.latitude, position.longitude,  height_over_sea_level) }
}