//! Windy webcams proxy (D102): `GET /api/webcams?bbox=north,east,south,west` → GeoJSON points.
//! The server holds the API key, and results are cached for 5 minutes, under the ~10 minute
//! expiry of free-tier image URLs.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

const API: &str = "https://api.windy.com/webcams/api/v3/webcams";
const TTL: Duration = Duration::from_secs(300);
/// Largest bbox side in degrees: the client asks from zoom 10, ~2.6° wide on a 1920 px screen.
pub const MAX_SPAN_DEG: f64 = 3.0;

pub struct Webcams {
    key: Option<String>,
    cache: Mutex<HashMap<String, (Instant, Value)>>,
}

#[derive(Debug, thiserror::Error)]
pub enum WebcamError {
    #[error("webcams are not configured (no WINDY_API_KEY)")]
    NoKey,
    #[error("bbox must be north,east,south,west with sides up to {MAX_SPAN_DEG}°")]
    BadBbox,
    #[error("webcam provider failed: {0}")]
    Upstream(String),
}

impl Webcams {
    pub fn new(key: Option<String>) -> Webcams {
        Webcams {
            key: key.filter(|k| !k.is_empty()),
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// Webcams in `bbox` ("north,east,south,west"), snapped out to a 0.05° grid so nearby
    /// views share a cache entry. Blocking: call from `spawn_blocking`.
    pub fn get(&self, bbox: &str) -> Result<Value, WebcamError> {
        let key = self.key.as_deref().ok_or(WebcamError::NoKey)?;
        let b = parse_bbox(bbox).ok_or(WebcamError::BadBbox)?;
        let id = b.map(|x| format!("{x:.2}")).join(",");
        let now = Instant::now();
        {
            let mut c = self.cache.lock().unwrap_or_else(|p| p.into_inner());
            c.retain(|_, (t, _)| now.duration_since(*t) < TTL);
            if let Some((_, v)) = c.get(&id) {
                return Ok(v.clone());
            }
        }
        // shortcut: one page of 50 (the API's max); dense areas show the 50 most popular. Page with offset if that's too few.
        let body: Value = ureq::get(API)
            .set("x-windy-api-key", key)
            .query("bbox", &id)
            .query("limit", "50")
            .query("sortKey", "popularity")
            .query("sortDirection", "desc")
            .query("include", "images,location,urls")
            .timeout(Duration::from_secs(10))
            .call()
            .map_err(|e| WebcamError::Upstream(e.to_string()))?
            .into_json()
            .map_err(|e| WebcamError::Upstream(e.to_string()))?;
        let v = to_geojson(&body);
        self.cache
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(id, (now, v.clone()));
        Ok(v)
    }
}

/// "north,east,south,west" → snapped out to 0.05°, or None when malformed or too large.
fn parse_bbox(s: &str) -> Option<[f64; 4]> {
    let v: Vec<f64> = s
        .split(',')
        .map(|x| x.trim().parse().ok())
        .collect::<Option<_>>()?;
    let [n, e, so, w] = <[f64; 4]>::try_from(v).ok()?;
    let ok = [n, e, so, w].iter().all(|x| x.is_finite())
        && (-90.0..=90.0).contains(&n)
        && (-90.0..=90.0).contains(&so)
        && n > so
        && e > w
        && n - so <= MAX_SPAN_DEG
        && e - w <= MAX_SPAN_DEG;
    let (up, down) = (
        |x: f64| (x * 20.0).ceil() / 20.0,
        |x: f64| (x * 20.0).floor() / 20.0,
    );
    ok.then(|| [up(n), up(e), down(so), down(w)])
}

/// Windy's `{ webcams: [...] }` as a GeoJSON FeatureCollection with id, title, image and page url.
/// The image is the largest the free tier returns among preview, thumbnail and icon.
fn to_geojson(body: &Value) -> Value {
    let features: Vec<Value> = body["webcams"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| {
            let (lat, lon) = (
                c["location"]["latitude"].as_f64()?,
                c["location"]["longitude"].as_f64()?,
            );
            let cur = &c["images"]["current"];
            let image = ["preview", "thumbnail", "icon"]
                .iter()
                .find_map(|k| cur[k].as_str());
            Some(json!({
                "type": "Feature",
                "geometry": { "type": "Point", "coordinates": [lon, lat] },
                "properties": {
                    "id": c["webcamId"],
                    "title": c["title"].as_str().unwrap_or("Webcam"),
                    "image": image,
                    "url": c["urls"]["detail"].as_str()?,
                },
            }))
        })
        .collect();
    json!({ "type": "FeatureCollection", "features": features })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windy_response_becomes_geojson() {
        let body = json!({ "total": 2, "webcams": [
            { "webcamId": 1, "title": "Wattwil – Ricken", "status": "active",
              "location": { "latitude": 47.3, "longitude": 9.09 },
              "images": { "current": { "icon": "i.jpg", "thumbnail": "t.jpg", "preview": "p.jpg" } },
              "urls": { "detail": "https://www.windy.com/webcams/1" } },
            { "webcamId": 2, "title": "No place", "urls": { "detail": "x" } },
        ]});
        let g = to_geojson(&body);
        let f = &g["features"];
        assert_eq!(
            f.as_array().unwrap().len(),
            1,
            "webcams without a location are dropped"
        );
        assert_eq!(f[0]["geometry"]["coordinates"], json!([9.09, 47.3]));
        assert_eq!(f[0]["properties"]["image"], "p.jpg");
        assert_eq!(f[0]["properties"]["url"], "https://www.windy.com/webcams/1");
    }

    #[test]
    fn bbox_is_validated_and_snapped_out() {
        assert_eq!(
            parse_bbox("47.33,9.12,47.27,9.04"),
            Some([47.35, 9.15, 47.25, 9.0])
        );
        assert_eq!(parse_bbox("47,9,48,8"), None, "south above north");
        assert_eq!(parse_bbox("48,12,46,8"), None, "too large");
        assert_eq!(parse_bbox("47,9,NaN,8"), None);
        assert_eq!(parse_bbox("47,9,46.5"), None);
    }

    #[test]
    fn no_key_is_an_error_not_a_request() {
        assert!(matches!(
            Webcams::new(None).get("47.33,9.12,47.27,9.04"),
            Err(WebcamError::NoKey)
        ));
        assert!(matches!(
            Webcams::new(Some(String::new())).get("1,2,0,1"),
            Err(WebcamError::NoKey)
        ));
    }
}
