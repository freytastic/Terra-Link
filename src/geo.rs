use maxminddb::geoip2::City;
use maxminddb::Reader;
use rand::Rng;
use std::net::IpAddr;

pub struct GeoResolver {
    reader: Option<Reader<Vec<u8>>>,
}

// Default implementation so App can #[derive(Default)]
impl Default for GeoResolver {
    fn default() -> Self {
        Self::new("GeoLite2-City.mmdb")
    }
}

impl GeoResolver {
    pub fn new(db_path: &str) -> Self {
        let reader = Reader::open_readfile(db_path).ok();
        if reader.is_none() {
            eprintln!(
                "Warning: Failed to load MaxMind DB at {}. Geospatial mapping offline.",
                db_path
            );
        }
        Self { reader }
    }

    pub fn get_fuzzed_location(&self, ip: IpAddr) -> Option<(f64, f64, String)> {
        // Fallback for local testing loopback
        if ip.is_loopback() {
            let mut rng = rand::thread_rng();
            // Random coordinate on earth
            let lat = rng.gen_range(-80.0..80.0);
            let lon = rng.gen_range(-180.0..180.0);
            return Some((lat, lon, "Localhost".to_string()));
        }

        if let Some(reader) = &self.reader {
            if let Ok(result) = reader.lookup(ip) {
                if let Ok(Some(city)) = result.decode::<City>() {
                    let location = city.location;
                    if let (Some(lat), Some(lon)) = (location.latitude, location.longitude) {
                        let mut place_name = String::new();
                        if let Some(en) = city.city.names.english {
                            place_name.push_str(en);
                        }

                        if let Some(iso) = city.country.iso_code {
                            if !place_name.is_empty() {
                                place_name.push_str(", ");
                            }
                            place_name.push_str(iso);
                        }

                        if place_name.is_empty() {
                            place_name = "Unknown".to_string();
                        }

                        let (f_lat, f_lon) = self.apply_fuzzing(lat, lon);
                        return Some((f_lat, f_lon, place_name));
                    }
                }
            }
        }
        None
    }

    fn apply_fuzzing(&self, lat: f64, lon: f64) -> (f64, f64) {
        let mut rng = rand::thread_rng();
        // Fuzz by +/- 0.5 degrees (~55km at equator) to preserve privacy
        let fuzzed_lat = lat + rng.gen_range(-0.5..0.5);
        let fuzzed_lon = lon + rng.gen_range(-0.5..0.5);
        (fuzzed_lat, fuzzed_lon)
    }
}
