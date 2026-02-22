use maxminddb::Reader;
use maxminddb::geoip2::City;
use std::net::IpAddr;
fn test_lookup(r: &Reader<Vec<u8>>, ip: IpAddr) -> Result<City, maxminddb::MaxMindDbError> {
    r.lookup(ip)
}
