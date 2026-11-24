//! Travel time (ADR 0002): a Saturday-morning public transport timetable built from
//! GTFS into `transit.bin` by `trail-calc-transit`. The server answers "where can these
//! travel origins get within their time budgets" with a connection scan and draws the
//! shared and partial reach as one PNG.

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::path::Path;

use bytemuck::{Pod, Zeroable};
use rstar::primitives::GeomWithData;
use rstar::RTree;

use crate::geodesy::LocalProjection;
use crate::graph::LatLon;

/// Walking speed, 4.5 km/h in a straight line.
pub const WALK_MPS: f64 = 4.5 / 3.6;
/// Longest walk from a travel origin to its first stop.
pub const ACCESS_S: u32 = 15 * 60;
/// Longest walk from the last stop (or from the origin alone) to a place.
pub const LAST_WALK_S: u32 = 20 * 60;
/// Transfer walks between stations.
pub const FOOTPATH_M: f64 = 400.0;
/// Changing vehicles at one station.
pub const CHANGE_S: u32 = 2 * 60;
/// Closest spacing of expanded frequency-trip departures.
pub const FREQ_MIN_S: u32 = 5 * 60;
/// Departures tried, every 10 min from 06:00 to 10:00.
pub const WINDOW: (u32, u32, u32) = (6 * 3600, 10 * 3600, 600);
pub const MAX_BUDGET_MIN: u32 = 240;
pub const MIN_BUDGET_MIN: u32 = 10;
pub const MAX_ORIGINS: usize = 8;
const MAGIC: &[u8; 8] = b"AYTRAN01";
const REF_LAT: f64 = 46.8;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Pod, Zeroable)]
pub struct Conn {
    pub from: u32,
    pub to: u32,
    pub dep: u32,
    pub arr: u32,
    pub trip: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Pod, Zeroable)]
pub struct Foot {
    pub to: u32,
    pub secs: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum GtfsError {
    #[error("{0}: {1}")]
    Csv(String, csv::Error),
    #[error("{0}: missing column {1}")]
    Column(String, &'static str),
    #[error("{0}: bad value {1:?}")]
    Value(String, String),
    #[error("no Saturday with service in the feed")]
    NoSaturday,
}

#[derive(Debug)]
pub struct Transit {
    /// Service date, YYYYMMDD.
    pub date: u32,
    pub stations: Vec<LatLon>,
    pub trips: u32,
    /// Sorted by departure.
    pub conns: Vec<Conn>,
    /// Footpaths of station i are `foot[foot_start[i]..foot_start[i + 1]]`.
    pub foot_start: Vec<u32>,
    pub foot: Vec<Foot>,
    tree: RTree<GeomWithData<[f64; 2], u32>>,
}

/// Seconds after midnight from `HH:MM:SS` (hours may pass 24).
fn secs(s: &str) -> Option<u32> {
    let mut it = s.trim().split(':').map(|p| p.parse::<u32>().ok());
    let (h, m, s) = (it.next()??, it.next()??, it.next()??);
    Some(h * 3600 + m * 60 + s)
}

/// Days since 1970-01-01 for YYYYMMDD (proleptic Gregorian).
fn days(ymd: u32) -> i64 {
    let (y, m, d) = (
        (ymd / 10000) as i64,
        (ymd / 100 % 100) as i64,
        (ymd % 100) as i64,
    );
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * ((m + 9) % 12) + 2) / 5 + d - 1;
    era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468
}

fn ymd(days: i64) -> u32 {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y * 10000 + m * 100 + d) as u32
}

fn is_saturday(day: i64) -> bool {
    day.rem_euclid(7) == 2 // 1970-01-01 was a Thursday
}

/// Calls `f` with the wanted columns of each row of a GTFS file ("" where absent).
fn rows(
    dir: &Path,
    name: &str,
    cols: &[&'static str],
    mut f: impl FnMut(&[&str]) -> Result<(), GtfsError>,
) -> Result<(), GtfsError> {
    let path = dir.join(name);
    if !path.exists() && matches!(name, "calendar_dates.txt" | "frequencies.txt") {
        return Ok(());
    }
    let err = |e| GtfsError::Csv(name.into(), e);
    let mut rd = csv::Reader::from_path(&path).map_err(err)?;
    let head = rd.headers().map_err(err)?.clone();
    let idx: Vec<Option<usize>> = cols
        .iter()
        .map(|c| {
            head.iter()
                .position(|h| h.trim_start_matches('\u{feff}') == *c)
        })
        .collect();
    // Only the first column of each file is required; the rest may be absent.
    if idx[0].is_none() {
        return Err(GtfsError::Column(name.into(), cols[0]));
    }
    let mut rec = csv::StringRecord::new();
    while rd.read_record(&mut rec).map_err(err)? {
        let vals: Vec<&str> = idx
            .iter()
            .map(|i| i.and_then(|i| rec.get(i)).unwrap_or(""))
            .collect();
        f(&vals)?;
    }
    Ok(())
}

impl Transit {
    /// Build from an unzipped GTFS feed. `date` (YYYYMMDD) overrides the Saturday choice:
    /// by default the Saturday with the most trips, so a holiday Saturday is never picked.
    pub fn build(dir: &Path, date: Option<u32>) -> Result<Transit, GtfsError> {
        let bad = |f: &str, v: &str| GtfsError::Value(f.into(), v.into());

        // Services per Saturday.
        let mut sat_services: HashMap<i64, Vec<String>> = HashMap::new();
        rows(
            dir,
            "calendar.txt",
            &["service_id", "saturday", "start_date", "end_date"],
            |r| {
                if r[1] != "1" {
                    return Ok(());
                }
                let p = |v: &str| v.parse::<u32>().map_err(|_| bad("calendar.txt", v));
                for d in days(p(r[2])?)..=days(p(r[3])?) {
                    if is_saturday(d) {
                        sat_services.entry(d).or_default().push(r[0].to_string());
                    }
                }
                Ok(())
            },
        )?;
        rows(
            dir,
            "calendar_dates.txt",
            &["service_id", "date", "exception_type"],
            |r| {
                let d = days(r[1].parse().map_err(|_| bad("calendar_dates.txt", r[1]))?);
                if !is_saturday(d) {
                    return Ok(());
                }
                let s = sat_services.entry(d).or_default();
                match r[2] {
                    "1" => s.push(r[0].to_string()),
                    "2" => s.retain(|x| x != r[0]),
                    v => return Err(bad("calendar_dates.txt", v)),
                }
                Ok(())
            },
        )?;

        let mut trips_of: HashMap<String, Vec<String>> = HashMap::new();
        rows(dir, "trips.txt", &["trip_id", "service_id"], |r| {
            trips_of
                .entry(r[1].to_string())
                .or_default()
                .push(r[0].to_string());
            Ok(())
        })?;
        let count = |d: &i64| -> usize {
            sat_services[d]
                .iter()
                .map(|s| trips_of.get(s).map_or(0, Vec::len))
                .sum()
        };
        let day = match date {
            Some(ymd) => days(ymd),
            None => *sat_services
                .keys()
                .max_by_key(|d| (count(d), std::cmp::Reverse(**d)))
                .ok_or(GtfsError::NoSaturday)?,
        };
        let mut trip_idx: HashMap<String, u32> = HashMap::new();
        for s in sat_services.get(&day).into_iter().flatten() {
            for t in trips_of.get(s).into_iter().flatten() {
                let n = trip_idx.len() as u32;
                trip_idx.entry(t.clone()).or_insert(n);
            }
        }
        if trip_idx.is_empty() {
            return Err(GtfsError::NoSaturday);
        }
        drop(trips_of);

        // Stations: platforms collapse onto their parent.
        let mut raw: Vec<(String, LatLon, String)> = Vec::new();
        rows(
            dir,
            "stops.txt",
            &["stop_id", "stop_lat", "stop_lon", "parent_station"],
            |r| {
                let f = |v: &str| v.trim().parse::<f64>().map_err(|_| bad("stops.txt", v));
                raw.push((
                    r[0].into(),
                    LatLon {
                        lat: f(r[1])?,
                        lon: f(r[2])?,
                    },
                    r[3].into(),
                ));
                Ok(())
            },
        )?;
        let pos: HashMap<&str, LatLon> = raw.iter().map(|(id, p, _)| (id.as_str(), *p)).collect();
        let mut station_of: HashMap<String, u32> = HashMap::new();
        let mut by_key: HashMap<&str, u32> = HashMap::new();
        let mut stations = Vec::new();
        for (id, p, parent) in &raw {
            let key = if parent.is_empty() || !pos.contains_key(parent.as_str()) {
                id.as_str()
            } else {
                parent.as_str()
            };
            let s = *by_key.entry(key).or_insert_with(|| {
                stations.push(pos.get(key).copied().unwrap_or(*p));
                stations.len() as u32 - 1
            });
            station_of.insert(id.clone(), s);
        }

        // Stop times of the day's trips, then consecutive pairs as connections.
        let mut st: Vec<(u32, u32, u32, u32, u32)> = Vec::new(); // trip, seq, arr, dep, station
        rows(
            dir,
            "stop_times.txt",
            &[
                "trip_id",
                "arrival_time",
                "departure_time",
                "stop_id",
                "stop_sequence",
            ],
            |r| {
                let Some(&t) = trip_idx.get(r[0]) else {
                    return Ok(());
                };
                let Some(&s) = station_of.get(r[3]) else {
                    return Err(bad("stop_times.txt", r[3]));
                };
                let arr = secs(r[1])
                    .or(secs(r[2]))
                    .ok_or_else(|| bad("stop_times.txt", r[1]))?;
                let dep = secs(r[2]).unwrap_or(arr);
                let seq = r[4].parse().map_err(|_| bad("stop_times.txt", r[4]))?;
                st.push((t, seq, arr, dep, s));
                Ok(())
            },
        )?;
        st.sort_unstable_by_key(|x| (x.0, x.1));
        let last_dep = WINDOW.1 + MAX_BUDGET_MIN * 60;

        // Frequency trips: their stop times are a pattern, repeated from start to end.
        let mut freq: HashMap<u32, Vec<(u32, u32, u32)>> = HashMap::new();
        rows(
            dir,
            "frequencies.txt",
            &["trip_id", "start_time", "end_time", "headway_secs"],
            |r| {
                let Some(&t) = trip_idx.get(r[0]) else {
                    return Ok(());
                };
                let v = |s: &str, x: Option<u32>| x.ok_or_else(|| bad("frequencies.txt", s));
                let h = v(r[3], r[3].parse().ok())?;
                freq.entry(t)
                    .or_default()
                    .push((v(r[1], secs(r[1]))?, v(r[2], secs(r[2]))?, h));
                Ok(())
            },
        )?;
        let mut trips = trip_idx.len() as u32;
        if !freq.is_empty() {
            let mut out = Vec::with_capacity(st.len());
            for g in st.chunk_by(|a, b| a.0 == b.0) {
                let Some(fs) = freq.get(&g[0].0) else {
                    out.extend_from_slice(g);
                    continue;
                };
                let base = g[0].3;
                for &(start, end, h) in fs {
                    // shortcut: departures at least FREQ_MIN_S apart (lifts list 60 s), so
                    // up to that much extra wait; fine for a planning overlay.
                    let mut t0 = start;
                    while t0 < end && t0 <= last_dep {
                        let at = |x: u32| (x + t0).saturating_sub(base);
                        out.extend(g.iter().map(|x| (trips, x.1, at(x.2), at(x.3), x.4)));
                        trips += 1;
                        t0 += h.max(FREQ_MIN_S);
                    }
                }
            }
            st = out;
        }
        let mut conns: Vec<Conn> = st
            .windows(2)
            .filter(|w| w[0].0 == w[1].0 && w[0].4 != w[1].4)
            .filter(|w| w[0].3 >= WINDOW.0 && w[0].3 <= last_dep)
            .map(|w| Conn {
                from: w[0].4,
                to: w[1].4,
                dep: w[0].3,
                arr: w[1].2.max(w[0].3),
                trip: w[0].0,
            })
            .collect();
        conns.sort_unstable_by_key(|c| (c.dep, c.arr));
        Ok(Transit::assemble(ymd(day), stations, trips, conns, None))
    }

    /// Index stations; compute footpaths unless they are given (when opening a file).
    fn assemble(
        date: u32,
        stations: Vec<LatLon>,
        trips: u32,
        conns: Vec<Conn>,
        foot: Option<(Vec<u32>, Vec<Foot>)>,
    ) -> Transit {
        let proj = LocalProjection::new(REF_LAT);
        let tree = RTree::bulk_load(
            stations
                .iter()
                .enumerate()
                .map(|(i, p)| GeomWithData::new(proj.project(*p), i as u32))
                .collect(),
        );
        let (foot_start, foot) = foot.unwrap_or_else(|| {
            let mut start = vec![0u32];
            let mut foot = Vec::new();
            for (i, p) in stations.iter().enumerate() {
                let xy = proj.project(*p);
                for n in tree.locate_within_distance(xy, FOOTPATH_M * FOOTPATH_M) {
                    if n.data as usize != i {
                        let d =
                            ((n.geom()[0] - xy[0]).powi(2) + (n.geom()[1] - xy[1]).powi(2)).sqrt();
                        foot.push(Foot {
                            to: n.data,
                            secs: (d / WALK_MPS).round() as u32,
                        });
                    }
                }
                start.push(foot.len() as u32);
            }
            (start, foot)
        });
        Transit {
            date,
            stations,
            trips,
            conns,
            foot_start,
            foot,
            tree,
        }
    }

    pub fn write(&self, path: &Path) -> io::Result<()> {
        let mut f = io::BufWriter::new(std::fs::File::create(path)?);
        f.write_all(MAGIC)?;
        let n = [
            self.date,
            self.stations.len() as u32,
            self.trips,
            self.conns.len() as u32,
            self.foot.len() as u32,
        ];
        f.write_all(bytemuck::cast_slice(&n))?;
        let ll: Vec<f64> = self.stations.iter().flat_map(|p| [p.lat, p.lon]).collect();
        f.write_all(bytemuck::cast_slice(&ll))?;
        f.write_all(bytemuck::cast_slice(&self.conns))?;
        f.write_all(bytemuck::cast_slice(&self.foot_start))?;
        f.write_all(bytemuck::cast_slice(&self.foot))?;
        f.flush()
    }

    pub fn open(path: &Path) -> io::Result<Transit> {
        let mut b = Vec::new();
        std::fs::File::open(path)?.read_to_end(&mut b)?;
        let bad = || io::Error::new(io::ErrorKind::InvalidData, "not a transit.bin");
        if b.len() < 28 || &b[..8] != MAGIC {
            return Err(bad());
        }
        let u = |i: usize| u32::from_le_bytes(b[i..i + 4].try_into().unwrap()) as usize;
        let (date, ns, trips, nc, nf) = (u(8) as u32, u(12), u(16) as u32, u(20), u(24));
        let sizes = [ns * 16, nc * 20, (ns + 1) * 4, nf * 8];
        if b.len() != 28 + sizes.iter().sum::<usize>() {
            return Err(bad());
        }
        let mut at = 28;
        let mut take = |len: usize| {
            let s = &b[at..at + len];
            at += len;
            s
        };
        // pod_collect_to_vec copies, so the unaligned file buffer is fine.
        let ll: Vec<f64> = bytemuck::pod_collect_to_vec(take(sizes[0]));
        let conns = bytemuck::pod_collect_to_vec(take(sizes[1]));
        let foot_start = bytemuck::pod_collect_to_vec(take(sizes[2]));
        let foot = bytemuck::pod_collect_to_vec(take(sizes[3]));
        let stations = ll
            .chunks(2)
            .map(|c| LatLon {
                lat: c[0],
                lon: c[1],
            })
            .collect();
        Ok(Transit::assemble(
            date,
            stations,
            trips,
            conns,
            Some((foot_start, foot)),
        ))
    }

    /// Fastest door-to-door seconds from `origin` to each station within `budget_s`, over
    /// departures in the window: `(station, secs)`.
    pub fn reach(&self, origin: LatLon, budget_s: u32) -> Vec<(u32, u32)> {
        let proj = LocalProjection::new(REF_LAT);
        let xy = proj.project(origin);
        let max_m = ACCESS_S as f64 * WALK_MPS;
        let access: Vec<(u32, u32)> = self
            .tree
            .locate_within_distance(xy, max_m * max_m)
            .map(|n| {
                let d = ((n.geom()[0] - xy[0]).powi(2) + (n.geom()[1] - xy[1]).powi(2)).sqrt();
                (n.data, (d / WALK_MPS).round() as u32)
            })
            .filter(|&(_, s)| s <= budget_s)
            .collect();
        let n = self.stations.len();
        let mut acc = vec![u32::MAX; n];
        for &(s, w) in &access {
            acc[s as usize] = w;
        }
        let mut best = vec![u32::MAX; n];
        let (mut arr, mut ready) = (vec![u32::MAX; n], vec![u32::MAX; n]);
        // Latest time to leave the origin for the journey to a stop; `walked` stops are
        // reached on foot from the origin, so their leave time is set when boarding.
        let (mut leave, mut walked) = (vec![0u32; n], vec![false; n]);
        let (mut on, mut trip_leave) = (
            vec![false; self.trips as usize],
            vec![0u32; self.trips as usize],
        );
        // shortcut: one scan per 10 min of departures, each tracking the leave time so a
        // trip off the grid costs no wait; a profile scan would be exact for later arrivals too.
        for t0 in (WINDOW.0..=WINDOW.1).step_by(WINDOW.2 as usize) {
            arr.fill(u32::MAX);
            ready.fill(u32::MAX);
            walked.fill(false);
            on.fill(false);
            for &(s, w) in &access {
                arr[s as usize] = t0 + w;
                ready[s as usize] = t0 + w;
                walked[s as usize] = true;
            }
            let end = t0 + WINDOW.2 + budget_s;
            let first = self.conns.partition_point(|c| c.dep < t0);
            for c in &self.conns[first..] {
                if c.dep > end {
                    break;
                }
                let (from, to, trip) = (c.from as usize, c.to as usize, c.trip as usize);
                if ready[from] <= c.dep {
                    let l = if walked[from] {
                        c.dep - acc[from]
                    } else {
                        leave[from]
                    };
                    if !on[trip] || l > trip_leave[trip] {
                        trip_leave[trip] = l;
                    }
                    on[trip] = true;
                } else if !on[trip] {
                    continue;
                }
                let l = trip_leave[trip];
                let better = c.arr < arr[to] || (c.arr == arr[to] && !walked[to] && l > leave[to]);
                if c.arr <= l + budget_s && better {
                    arr[to] = c.arr;
                    ready[to] = ready[to].min(c.arr + CHANGE_S);
                    (leave[to], walked[to]) = (l, false);
                    for f in
                        &self.foot[self.foot_start[to] as usize..self.foot_start[to + 1] as usize]
                    {
                        let (t, g) = (c.arr + f.secs, f.to as usize);
                        if t < arr[g] {
                            arr[g] = t;
                            ready[g] = ready[g].min(t);
                            (leave[g], walked[g]) = (l, false);
                        }
                    }
                }
            }
            for s in 0..n {
                if arr[s] != u32::MAX {
                    let t = arr[s] - if walked[s] { t0 } else { leave[s] };
                    best[s] = best[s].min(t);
                }
            }
        }
        best.iter()
            .enumerate()
            .filter(|(_, &b)| b <= budget_s)
            .map(|(i, &b)| (i as u32, b))
            .collect()
    }

    /// Per raster cell, how many of the origins `(point, budget minutes)` reach it.
    /// The origin (0 s) and every station it reaches within `minutes`, with the
    /// door-to-door seconds: what the client draws and hovers from (ADR 0005).
    pub fn stops(&self, origin: LatLon, minutes: u32) -> Vec<(LatLon, u32)> {
        let mut v = vec![(origin, 0)];
        v.extend(
            self.reach(origin, minutes * 60)
                .into_iter()
                .map(|(s, t)| (self.stations[s as usize], t)),
        );
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Transit {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/gtfs");
        Transit::build(&dir, None).unwrap()
    }

    fn st(t: &Transit, lon: f64) -> u32 {
        t.stations
            .iter()
            .position(|p| (p.lon - lon).abs() < 1e-9)
            .unwrap() as u32
    }

    #[test]
    fn dates_round_trip_and_saturdays() {
        assert_eq!(ymd(days(20261219)), 20261219);
        assert!(is_saturday(days(20261219)));
        assert!(!is_saturday(days(20261220)));
        assert_eq!(secs("25:01:02"), Some(90062));
    }

    #[test]
    fn picks_the_busiest_saturday_and_its_trips_only() {
        let t = fixture();
        // The 12th drops SAT, the 19th adds EXTRA on top of it.
        assert_eq!(t.date, 20261219);
        // Platforms collapse onto A and B; C and D stand alone.
        assert_eq!(t.stations.len(), 4);
        // T1 two legs, T2 and T4 one each, T6 hourly from 06 to 09 (3); T3 is weekday, T5 before
        // the window.
        assert_eq!(t.conns.len(), 7);
        // Ids: five trips that day, plus three T6 runs (its pattern id stays unused).
        assert_eq!(t.trips, 5 + 3);
        assert!(t.conns.windows(2).all(|w| w[0].dep <= w[1].dep));
        let (c, d) = (st(&t, 8.3) as usize, st(&t, 8.3025));
        let fp = &t.foot[t.foot_start[c] as usize..t.foot_start[c + 1] as usize];
        assert_eq!(fp.len(), 1);
        assert_eq!(fp[0].to, d);
        let a = st(&t, 8.0) as usize;
        assert_eq!(t.foot_start[a], t.foot_start[a + 1]);
    }

    #[test]
    fn reach_takes_the_fastest_trip_in_the_window() {
        let t = fixture();
        let a = LatLon {
            lat: 47.0,
            lon: 8.0,
        };
        let r: HashMap<u32, u32> = t.reach(a, 30 * 60).into_iter().collect();
        assert_eq!(r[&st(&t, 8.1)], 10 * 60);
        // The 08:04 express beats T1's 30 min, with no wait though it is off the 10 min grid.
        assert_eq!(r[&st(&t, 8.3)], 15 * 60);
        // C plus the ~190 m walk to D.
        let d = r[&st(&t, 8.3025)];
        assert!((15 * 60 + 140..15 * 60 + 170).contains(&d), "{d}");
        let r: HashMap<u32, u32> = t.reach(a, 12 * 60).into_iter().collect();
        assert!(r.contains_key(&st(&t, 8.1)));
        assert!(!r.contains_key(&st(&t, 8.3)));
    }

    #[test]
    fn stops_start_with_the_origin_and_stay_in_budget() {
        let t = fixture();
        let a = LatLon {
            lat: 47.0,
            lon: 8.0,
        };
        let s = t.stops(a, 12);
        assert_eq!(s[0], (a, 0));
        assert!(s.iter().all(|&(_, secs)| secs <= 12 * 60));
        assert!(s.iter().any(|&(p, _)| (p.lon - 8.1).abs() < 1e-6));
        assert!(!s.iter().any(|&(p, _)| (p.lon - 8.3).abs() < 1e-6));
    }

    #[test]
    fn file_round_trip() {
        let t = fixture();
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("transit.bin");
        t.write(&p).unwrap();
        let u = Transit::open(&p).unwrap();
        assert_eq!(
            (u.date, u.trips, &u.conns, &u.foot, &u.foot_start),
            (t.date, t.trips, &t.conns, &t.foot, &t.foot_start)
        );
        assert_eq!(u.stations.len(), t.stations.len());
    }
}
