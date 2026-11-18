//! Small geodesy helpers: haversine distance and a local metric projection.

use crate::graph::LatLon;

pub const EARTH_RADIUS_M: f64 = 6_371_008.8;

pub fn haversine_m(a: LatLon, b: LatLon) -> f64 {
    let (la, lb) = (a.lat.to_radians(), b.lat.to_radians());
    let dlat = lb - la;
    let dlon = (b.lon - a.lon).to_radians();
    let h = (dlat / 2.0).sin().powi(2) + la.cos() * lb.cos() * (dlon / 2.0).sin().powi(2);
    2.0 * EARTH_RADIUS_M * h.sqrt().min(1.0).asin()
}

/// Equirectangular projection around a reference latitude; metres. Accurate enough
/// for nearest-neighbour ordering and point-to-segment distances over tens of km.
#[derive(Debug, Clone, Copy)]
pub struct LocalProjection {
    cos_lat0: f64,
}

impl LocalProjection {
    pub fn new(ref_lat: f64) -> Self {
        LocalProjection {
            cos_lat0: ref_lat.to_radians().cos(),
        }
    }

    pub fn project(&self, p: LatLon) -> [f64; 2] {
        [
            p.lon.to_radians() * self.cos_lat0 * EARTH_RADIUS_M,
            p.lat.to_radians() * EARTH_RADIUS_M,
        ]
    }

    pub fn unproject(&self, xy: [f64; 2]) -> LatLon {
        // cos_lat0 is 0 only at the poles; guard against division by zero.
        let c = self.cos_lat0.max(1e-9);
        LatLon {
            lat: (xy[1] / EARTH_RADIUS_M).to_degrees(),
            lon: (xy[0] / (EARTH_RADIUS_M * c)).to_degrees(),
        }
    }
}

/// Closest point on segment ab to p in projected metres: (point, t in 0..=1 along ab).
pub fn closest_on_segment(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> ([f64; 2], f64) {
    let d = [b[0] - a[0], b[1] - a[1]];
    let len2 = d[0] * d[0] + d[1] * d[1];
    if len2 <= 0.0 {
        return (a, 0.0);
    }
    let t = (((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / len2).clamp(0.0, 1.0);
    ([a[0] + t * d[0], a[1] + t * d[1]], t)
}

/// Haversine length of a polyline of (lat, lon, _) vertices.
pub fn polyline_length_m(points: impl IntoIterator<Item = (f64, f64, f32)>) -> f64 {
    let mut prev: Option<LatLon> = None;
    let mut total = 0.0;
    for (lat, lon, _) in points {
        let cur = LatLon { lat, lon };
        if let Some(p) = prev {
            total += haversine_m(p, cur);
        }
        prev = Some(cur);
    }
    total
}

/// Forward gain and loss with a 2 m deadband: a rise or fall counts once it
/// exceeds 2 m from the last turning point.
pub fn gain_loss(z: &[f32]) -> (f64, f64) {
    const DEADBAND: f64 = 2.0;
    let Some(&z0) = z.first() else {
        return (0.0, 0.0);
    };
    let (mut gain, mut loss) = (0.0, 0.0);
    let (mut turn, mut ext, mut dir) = (z0 as f64, z0 as f64, 0i8);
    for &z in &z[1..] {
        let z = z as f64;
        match dir {
            0 if z - turn > DEADBAND => (dir, ext) = (1, z),
            0 if turn - z > DEADBAND => (dir, ext) = (-1, z),
            1 if z > ext => ext = z,
            1 if ext - z > DEADBAND => {
                gain += ext - turn;
                (turn, ext, dir) = (ext, z, -1);
            }
            -1 if z < ext => ext = z,
            -1 if z - ext > DEADBAND => {
                loss += turn - ext;
                (turn, ext, dir) = (ext, z, 1);
            }
            _ => {}
        }
    }
    match dir {
        1 => gain += ext - turn,
        -1 => loss += turn - ext,
        _ => {}
    }
    (gain, loss)
}
