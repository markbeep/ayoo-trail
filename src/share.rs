//! Share links read on the server (D95, ADR 0006): the client's `?s=…&d=…` query turned back
//! into the `/api/generate` body the client would send, so link previews can show the loop.
//! Mirrors `web/src/url.ts` `decodeShare` and `web/src/api.ts`; range checks stay with the API.

use serde_json::{json, Value};
use std::collections::HashMap;

/// Weight order in `w=`, as in `url.ts`.
const WEIGHTS: [&str; 11] = [
    "slope",
    "uphill",
    "downhill",
    "surface",
    "tech",
    "technical",
    "official",
    "forest",
    "urban",
    "traffic",
    "view",
];
const PENALTIES: [&str; 5] = [
    "repeat",
    "avoid_factor",
    "prefer_factor",
    "stem_m",
    "max_overlap",
];

/// A shared loop: how to regenerate it and which one was selected.
#[derive(Debug)]
pub struct SharedLoop {
    /// Body for `POST /api/generate`.
    pub generate: Value,
    /// Index of the selected loop, best first (D79).
    pub index: usize,
    /// Dragged waypoints of an edited copy, as `[{lat, lon}]`.
    pub wps: Option<Value>,
    /// `climb_target_m` for `/api/route` when re-routing the copy.
    pub climb_target_m: Option<f64>,
    pub km: f64,
}

fn num(v: &str) -> Option<f64> {
    v.trim().parse::<f64>().ok().filter(|n| n.is_finite())
}

fn point(v: &str) -> Option<Value> {
    let mut it = v.split(',');
    let (lat, lon) = (num(it.next()?)?, num(it.next()?)?);
    Some(json!({ "lat": lat, "lon": lon }))
}

/// The shared loop in a query, or None when there is none or it doesn't parse.
pub fn decode(q: &HashMap<String, String>) -> Option<SharedLoop> {
    let start = point(q.get("s")?)?;
    let km = num(q.get("d")?)?;
    let strictness = num(q.get("t")?)?;
    let max_sac = num(q.get("x")?)?.round().clamp(0.0, 255.0) as u8;
    let w: Vec<&str> = q.get("w").map_or(vec![], |w| w.split(',').collect());
    let mut weights = serde_json::Map::new();
    for (i, k) in WEIGHTS.iter().enumerate() {
        let v = match w.get(i).filter(|s| !s.is_empty()) {
            Some(s) => num(s)?,
            None => match *k {
                "uphill" | "view" => 0.0,
                "downhill" | "technical" => -1.0,
                _ => 1.0,
            },
        };
        weights.insert((*k).into(), json!(v));
    }
    let climb_target_m = match q.get("c").map(String::as_str).unwrap_or("") {
        "max" | "off" => None,
        "gentle" => Some(30.0 * km),
        "hilly" => Some(60.0 * km),
        "mountain" => Some(90.0 * km),
        c => Some(num(c)?),
    };
    let climb = match q.get("c").map(String::as_str) {
        Some("off") => Value::Null,
        _ => json!({ "target_m": climb_target_m, "strictness": strictness }),
    };
    let mut exclude = vec![];
    for z in q.get("z").into_iter().flat_map(|z| z.split(';')) {
        let a: Vec<&str> = z.split(',').collect();
        let mode = match a.get(3).copied() {
            None => "penalize",
            Some("A") => "block",
            Some("p") => "prefer",
            Some("P") => "must",
            Some(_) => return None,
        };
        exclude.push(json!({ "lat": num(a[0])?, "lon": num(a.get(1)?)?, "radius_m": num(a.get(2)?)?, "mode": mode }));
    }
    // Only ways on swisstopo's map unless `v=0` (ADR 0007).
    let tlm_only = q.get("v").map(String::as_str) != Some("0");
    let mut profile =
        json!({ "weights": weights, "max_sac": max_sac, "exclude": exclude, "tlm_only": tlm_only });
    if let Some(n) = q.get("n") {
        let n: Vec<&str> = n.split(',').collect();
        for (i, k) in PENALTIES.iter().enumerate() {
            profile[*k] = json!(num(n.get(i)?)?);
        }
    }
    let wps = match q.get("p") {
        Some(p) => Some(Value::Array(
            p.split(';').map(point).collect::<Option<_>>()?,
        )),
        None => None,
    };
    Some(SharedLoop {
        generate: json!({ "start": start, "distance_m": km * 1000.0, "climb": climb, "profile": profile }),
        index: num(q.get("l").map_or("0", String::as_str))?
            .round()
            .clamp(0.0, 50.0) as usize,
        wps,
        climb_target_m,
        km,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(s: &str) -> HashMap<String, String> {
        s.split('&')
            .filter_map(|kv| kv.split_once('='))
            .map(|(k, v)| (k.into(), v.into()))
            .collect()
    }

    #[test]
    fn decodes_the_client_link_into_a_generate_body() {
        let s = decode(&q("s=47.2985,9.095&d=15&c=hilly&t=0.8&x=3&w=1,0,-1,1,1,-1,1,2.5,1,0&l=2&z=47.31,9.1,300;47.32,9.11,200,P&n=5,5,0.5,800,0.5&p=47.3,9.1;47.31,9.12"))
            .unwrap();
        let g = &s.generate;
        assert_eq!(g["start"], json!({"lat": 47.2985, "lon": 9.095}));
        assert_eq!(g["distance_m"], json!(15000.0));
        assert_eq!(g["climb"], json!({"target_m": 900.0, "strictness": 0.8}));
        assert_eq!(g["profile"]["max_sac"], json!(3));
        assert_eq!(g["profile"]["weights"]["forest"], json!(2.5));
        // An older link without the view weight gets its default.
        assert_eq!(g["profile"]["weights"]["view"], json!(0.0));
        assert_eq!(g["profile"]["exclude"][1]["mode"], json!("must"));
        assert_eq!(g["profile"]["exclude"][0]["mode"], json!("penalize"));
        assert_eq!(g["profile"]["stem_m"], json!(800.0));
        assert_eq!(s.index, 2);
        assert_eq!(s.wps.unwrap().as_array().unwrap().len(), 2);
        assert_eq!(s.climb_target_m, Some(900.0));
    }

    #[test]
    fn climb_off_and_max_and_bad_links() {
        let base = "s=47.3,9.1&d=10&t=1&x=3";
        assert_eq!(
            decode(&q(&format!("{base}&c=off"))).unwrap().generate["climb"],
            Value::Null
        );
        let m = decode(&q(&format!("{base}&c=max"))).unwrap();
        assert_eq!(m.generate["climb"]["target_m"], Value::Null);
        assert_eq!(m.climb_target_m, None);
        assert!(decode(&q("d=10&c=max&t=1&x=3")).is_none());
        assert!(decode(&q(&format!("{base}&c=max&z=47.3,9.1,300,q"))).is_none());
        assert!(decode(&q(&format!("{base}&c=max&w=1,x"))).is_none());
    }

    #[test]
    fn swisstopo_filter_is_on_unless_v_is_0() {
        let base = "s=47.3,9.1&d=10&t=1&x=3&c=max";
        let tlm = |s: &str| decode(&q(s)).unwrap().generate["profile"]["tlm_only"].clone();
        assert_eq!(tlm(base), json!(true));
        assert_eq!(tlm(&format!("{base}&v=0")), json!(false));
        assert_eq!(tlm(&format!("{base}&v=1")), json!(true));
    }
}
