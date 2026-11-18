//! WGS84 ↔ Swiss grid (CH1903+/LV95, CH1903/LV03) with the official swisstopo
//! approximate formulas (sub-metre accuracy inside Switzerland), no PROJ.

/// (lat, lon) degrees → (E, N) LV95 metres.
pub fn wgs84_to_lv95(lat: f64, lon: f64) -> (f64, f64) {
    let p = (lat * 3600.0 - 169_028.66) / 10_000.0;
    let l = (lon * 3600.0 - 26_782.5) / 10_000.0;
    let e =
        2_600_072.37 + 211_455.93 * l - 10_938.51 * l * p - 0.36 * l * p * p - 44.54 * l * l * l;
    let n = 1_200_147.07 + 308_807.95 * p + 3_745.25 * l * l + 76.63 * p * p - 194.56 * l * l * p
        + 119.79 * p * p * p;
    (e, n)
}

/// (E, N) LV95 metres → (lat, lon) degrees.
pub fn lv95_to_wgs84(e: f64, n: f64) -> (f64, f64) {
    let y = (e - 2_600_000.0) / 1_000_000.0;
    let x = (n - 1_200_000.0) / 1_000_000.0;
    let l =
        2.677_909_4 + 4.728_982 * y + 0.791_484 * y * x + 0.1306 * y * x * x - 0.0436 * y * y * y;
    let p = 16.902_389_2 + 3.238_272 * x
        - 0.270_978 * y * y
        - 0.002_528 * x * x
        - 0.0447 * y * y * x
        - 0.0140 * x * x * x;
    let (mut lat, mut lon) = (p * 100.0 / 36.0, l * 100.0 / 36.0);
    // The two approximations disagree by up to ~1 m; refine against the forward
    // formula so that a round trip is exact (metres per degree as the step).
    for _ in 0..3 {
        let (e1, n1) = wgs84_to_lv95(lat, lon);
        lat += (n - n1) / 111_200.0;
        lon += (e - e1) / (111_320.0 * lat.to_radians().cos());
    }
    (lat, lon)
}

/// (lat, lon) degrees → (y, x) LV03 metres.
pub fn wgs84_to_lv03(lat: f64, lon: f64) -> (f64, f64) {
    let (e, n) = wgs84_to_lv95(lat, lon);
    (e - 2_000_000.0, n - 1_000_000.0)
}
