//! OSM stage of the importer: walkable ways become graph edges; land-cover
//! polygons, peaks and viewpoints are returned for the later stages.
//!
//! XML (fixtures) and PBF (real extracts) feed the same three passes:
//! relations → ways → nodes, which is the order a PBF stores them in.

use std::collections::HashMap;
use std::path::Path;

use geo::{BoundingRect, Intersects, LineString, Point, Polygon};

use super::ImportError;
use crate::geodesy::polyline_length_m;
use crate::graph::{flags, Bbox, EdgeAttrs, GraphWriter, Highway, LatLon, Surface};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct ImportStats {
    pub nodes: u64,
    pub edges: u64,
    pub ways_used: u64,
    pub ways_skipped: u64,
    pub rings_failed: u64,
}

#[derive(Debug, Default, Clone)]
pub struct OsmExtras {
    /// x = lon, y = lat; holes kept as interiors.
    pub forest: Vec<Polygon<f64>>,
    /// Settlement polygons (landuse residential, commercial, industrial, retail; D77).
    pub residential: Vec<Polygon<f64>>,
    /// `natural=peak` with `ele` (1.0 when missing or unparseable).
    pub peaks: Vec<(LatLon, f32)>,
    /// `tourism=viewpoint`.
    pub viewpoints: Vec<LatLon>,
}

type Tags<'a> = Vec<(&'a str, &'a str)>;

enum Elem<'a> {
    Node {
        id: i64,
        lat: f64,
        lon: f64,
        tags: Tags<'a>,
    },
    Way {
        id: i64,
        refs: Vec<i64>,
        tags: Tags<'a>,
    },
    /// Way members only, as (way id, role).
    Relation {
        members: Vec<(i64, &'a str)>,
        tags: Tags<'a>,
    },
}

/// One full read of the input, handing every element to the callback.
type Pass<'s> = dyn FnMut(&mut dyn FnMut(Elem<'_>)) -> Result<(), ImportError> + 's;

/// `.pbf` is read with osmpbf (streamed, once per pass); anything else is XML,
/// loaded whole. `bbox = None` keeps everything.
pub fn import_osm(
    path: &Path,
    bbox: Option<Bbox>,
    osm_date: i64,
) -> Result<(GraphWriter, ImportStats, OsmExtras), ImportError> {
    if path.extension().is_some_and(|e| e == "pbf") {
        run(&mut |f| pbf_pass(path, f), bbox, osm_date)
    } else {
        let text = std::fs::read_to_string(path)?;
        let doc = roxmltree::Document::parse(&text).map_err(|e| ImportError::Xml(e.to_string()))?;
        run(&mut |f| xml_pass(&doc, f), bbox, osm_date)
    }
}

fn pbf_pass(path: &Path, f: &mut dyn FnMut(Elem<'_>)) -> Result<(), ImportError> {
    use osmpbf::{Element, RelMemberType};
    let pbf = |e: osmpbf::Error| ImportError::Pbf(e.to_string());
    let reader = osmpbf::ElementReader::from_path(path).map_err(pbf)?;
    reader
        .for_each(|e| match e {
            Element::Node(n) => f(Elem::Node {
                id: n.id(),
                lat: n.lat(),
                lon: n.lon(),
                tags: n.tags().collect(),
            }),
            Element::DenseNode(n) => f(Elem::Node {
                id: n.id(),
                lat: n.lat(),
                lon: n.lon(),
                tags: n.tags().collect(),
            }),
            Element::Way(w) => f(Elem::Way {
                id: w.id(),
                refs: w.refs().collect(),
                tags: w.tags().collect(),
            }),
            Element::Relation(r) => f(Elem::Relation {
                members: r
                    .members()
                    .filter(|m| m.member_type == RelMemberType::Way)
                    .map(|m| (m.member_id, m.role().unwrap_or("")))
                    .collect(),
                tags: r.tags().collect(),
            }),
        })
        .map_err(pbf)
}

fn xml_pass(doc: &roxmltree::Document, f: &mut dyn FnMut(Elem<'_>)) -> Result<(), ImportError> {
    fn num<T: std::str::FromStr>(n: roxmltree::Node, k: &str) -> Result<T, ImportError> {
        n.attribute(k).and_then(|v| v.parse().ok()).ok_or_else(|| {
            let id = n.attribute("id").unwrap_or("?");
            ImportError::Format(format!(
                "{} {id}: missing or bad `{k}`",
                n.tag_name().name()
            ))
        })
    }
    for n in doc.root_element().children().filter(|n| n.is_element()) {
        let children = |name: &'static str| n.children().filter(move |c| c.has_tag_name(name));
        let tags = || {
            children("tag")
                .filter_map(|c| Some((c.attribute("k")?, c.attribute("v")?)))
                .collect()
        };
        match n.tag_name().name() {
            "node" => f(Elem::Node {
                id: num(n, "id")?,
                lat: num(n, "lat")?,
                lon: num(n, "lon")?,
                tags: tags(),
            }),
            "way" => f(Elem::Way {
                id: num(n, "id")?,
                refs: children("nd")
                    .map(|c| num(c, "ref"))
                    .collect::<Result<_, _>>()?,
                tags: tags(),
            }),
            "relation" => f(Elem::Relation {
                members: children("member")
                    .filter(|c| c.attribute("type") == Some("way"))
                    .map(|c| Ok((num(c, "ref")?, c.attribute("role").unwrap_or(""))))
                    .collect::<Result<_, ImportError>>()?,
                tags: tags(),
            }),
            _ => {}
        }
    }
    Ok(())
}

fn tag<'a>(tags: &[(&str, &'a str)], k: &str) -> Option<&'a str> {
    tags.iter().find(|t| t.0 == k).map(|t| t.1)
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Cover {
    Forest,
    Residential,
}

fn cover(tags: &[(&str, &str)]) -> Option<Cover> {
    let landuse = tag(tags, "landuse");
    if landuse == Some("forest") || tag(tags, "natural") == Some("wood") {
        Some(Cover::Forest)
    } else if matches!(
        landuse,
        Some("residential" | "commercial" | "industrial" | "retail")
    ) {
        Some(Cover::Residential)
    } else {
        None
    }
}

/// The highway class, or `None` when the way is not walkable.
fn walkable(highway: &str, tags: &[(&str, &str)]) -> Option<Highway> {
    let h = match highway {
        "path" => Highway::Path,
        "footway" => Highway::Footway,
        "bridleway" => Highway::Bridleway,
        "steps" => Highway::Steps,
        "track" => Highway::Track,
        "cycleway" => Highway::Cycleway,
        "pedestrian" => Highway::Pedestrian,
        "living_street" => Highway::LivingStreet,
        "service" => Highway::Service,
        "residential" => Highway::Residential,
        "unclassified" => Highway::Unclassified,
        "tertiary" | "tertiary_link" => Highway::Tertiary,
        "secondary" | "secondary_link" => Highway::Secondary,
        "primary" | "primary_link" => Highway::Primary,
        _ => return None,
    };
    let foot = tag(tags, "foot");
    if tag(tags, "area") == Some("yes") || foot == Some("no") {
        return None;
    }
    if matches!(tag(tags, "access"), Some("no" | "private"))
        && !matches!(foot, Some("yes" | "designated" | "permissive"))
    {
        return None;
    }
    Some(h)
}

/// Tag → attribute mapping (D31). `length_m` is filled per edge.
fn edge_attrs(highway: Highway, tags: &[(&str, &str)]) -> EdgeAttrs {
    use Surface as S;
    let mut f = 0;
    let surface = match tag(tags, "surface") {
        Some("asphalt") => Some(S::Asphalt),
        Some(
            "concrete" | "concrete:plates" | "concrete:lanes" | "paving_stones" | "sett"
            | "cobblestone" | "unhewn_cobblestone" | "paved" | "metal" | "wood",
        ) => Some(S::Paved),
        Some("compacted" | "fine_gravel") => Some(S::Compacted),
        Some("gravel" | "pebblestone" | "unpaved") => Some(S::Gravel),
        Some("dirt" | "earth" | "ground" | "mud" | "sand" | "woodchips") => Some(S::Dirt),
        Some("grass" | "grass_paver") => Some(S::Grass),
        Some("rock" | "stone") => Some(S::Rock),
        _ => None,
    };
    let surface = surface.unwrap_or_else(|| {
        let s = match highway {
            Highway::Path
            | Highway::Footway
            | Highway::Bridleway
            | Highway::Steps
            | Highway::Cycleway
            | Highway::Other => S::Unknown,
            Highway::Track => match tag(tags, "tracktype") {
                Some("grade1") => S::Paved,
                Some("grade2") => S::Compacted,
                Some("grade4") => S::Dirt,
                Some("grade5") => S::Grass,
                _ => S::Gravel,
            },
            _ => S::Asphalt,
        };
        if s != S::Unknown {
            f |= flags::SURFACE_INFERRED;
        }
        s
    });
    let sac = match tag(tags, "sac_scale") {
        Some("hiking") => 1,
        Some("mountain_hiking") => 2,
        Some("demanding_mountain_hiking") => 3,
        Some("alpine_hiking") => 4,
        Some("demanding_alpine_hiking") => 5,
        Some("difficult_alpine_hiking") => 6,
        _ => 0,
    };
    if sac == 0 && matches!(highway, Highway::Path | Highway::Bridleway) {
        f |= flags::MISSING_TAGS;
    }
    let present = |k| tag(tags, k).is_some_and(|v| v != "no");
    if present("bridge") {
        f |= flags::BRIDGE;
    }
    if present("tunnel") {
        f |= flags::TUNNEL;
    }
    if tag(tags, "informal") == Some("yes") {
        f |= flags::INFORMAL;
    }
    EdgeAttrs {
        surface,
        sac,
        highway,
        flags: f,
        ..EdgeAttrs::default()
    }
}

fn run(
    pass: &mut Pass<'_>,
    bbox: Option<Bbox>,
    osm_date: i64,
) -> Result<(GraphWriter, ImportStats, OsmExtras), ImportError> {
    let mut stats = ImportStats::default();
    let mut extras = OsmExtras::default();
    let inside = |lat: f64, lon: f64| {
        bbox.is_none_or(|b| {
            (b.min_lat..=b.max_lat).contains(&lat) && (b.min_lon..=b.max_lon).contains(&lon)
        })
    };

    // Pass 1: land-cover multipolygons, as (cover, [(way id, is_inner)]).
    let mut relations: Vec<(Cover, Vec<(i64, bool)>)> = Vec::new();
    pass(&mut |e| {
        if let Elem::Relation { members, tags } = e {
            if let (Some("multipolygon"), Some(c)) = (tag(&tags, "type"), cover(&tags)) {
                let m = members.into_iter().map(|(id, role)| (id, role == "inner"));
                relations.push((c, m.collect()));
            }
        }
    })?;
    let mut member_ids: Vec<i64> = relations
        .iter()
        .flat_map(|r| r.1.iter().map(|m| m.0))
        .collect();
    member_ids.sort_unstable();
    member_ids.dedup();

    // Pass 2: walkable ways, closed land-cover ways, multipolygon member ways.
    let mut ways: Vec<(Vec<i64>, EdgeAttrs)> = Vec::new();
    let mut cover_ways: Vec<(Cover, Vec<i64>)> = Vec::new();
    let mut member_refs: HashMap<i64, Vec<i64>> = HashMap::new();
    pass(&mut |e| {
        let Elem::Way { id, refs, tags } = e else {
            return;
        };
        if member_ids.binary_search(&id).is_ok() {
            member_refs.insert(id, refs.clone());
        }
        if let Some(c) = cover(&tags) {
            if refs.len() >= 4 && refs.first() == refs.last() {
                cover_ways.push((c, refs.clone()));
            }
        }
        if let Some(hw) = tag(&tags, "highway") {
            match walkable(hw, &tags) {
                Some(h) => ways.push((refs, edge_attrs(h, &tags))),
                None => stats.ways_skipped += 1,
            }
        }
    })?;

    // Coordinates only for referenced nodes: sorted ids + parallel positions.
    // shortcut: the id list holds every ref before dedup (~8 B each), fine for CH; upgrade to chunked sort if a larger extract runs out of memory.
    let mut ids: Vec<i64> = ways.iter().flat_map(|w| &w.0).copied().collect();
    ids.extend(cover_ways.iter().flat_map(|w| &w.1));
    ids.extend(member_refs.values().flatten());
    ids.sort_unstable();
    ids.dedup();
    ids.shrink_to_fit();
    let mut pos = vec![[f32::NAN; 2]; ids.len()];

    // Pass 3: node coordinates, peaks, viewpoints.
    pass(&mut |e| {
        let Elem::Node { id, lat, lon, tags } = e else {
            return;
        };
        if let Ok(i) = ids.binary_search(&id) {
            pos[i] = [lat as f32, lon as f32];
        }
        if tags.is_empty() || !inside(lat, lon) {
            return;
        }
        let p = LatLon { lat, lon };
        if tag(&tags, "natural") == Some("peak") {
            let ele = tag(&tags, "ele").and_then(|v| v.parse::<f32>().ok());
            extras
                .peaks
                .push((p, ele.filter(|v| v.is_finite()).unwrap_or(1.0)));
        }
        if tag(&tags, "tourism") == Some("viewpoint") {
            extras.viewpoints.push(p);
        }
    })?;

    let resolve = |id: i64| ids.binary_search(&id).ok().filter(|&i| !pos[i][0].is_nan());

    // Resolve walkable ways to node-table indices and apply the bbox.
    let mut kept: Vec<(Vec<u32>, EdgeAttrs)> = Vec::new();
    for (refs, attrs) in ways {
        let mut v: Vec<u32> = refs
            .iter()
            .filter_map(|&id| resolve(id))
            .map(|i| i as u32)
            .collect();
        v.dedup();
        let touches = v.iter().any(|&i| {
            let [lat, lon] = pos[i as usize];
            inside(lat as f64, lon as f64)
        });
        if v.len() < 2 || !touches {
            stats.ways_skipped += 1;
            continue;
        }
        kept.push((v, attrs));
    }
    stats.ways_used = kept.len() as u64;

    // A node is a split point when it is used twice: shared, or repeated in one way.
    let mut uses = vec![0u8; ids.len()];
    for (v, _) in &kept {
        for &i in v {
            uses[i as usize] = uses[i as usize].saturating_add(1);
        }
    }
    let mut w = GraphWriter::new(osm_date);
    let mut gid = vec![u32::MAX; ids.len()];
    for (v, attrs) in &kept {
        let mut start = 0;
        for k in 1..v.len() {
            if k == v.len() - 1 || uses[v[k] as usize] >= 2 {
                add_segment(&mut w, &mut gid, &pos, &v[start..=k], *attrs);
                start = k;
            }
        }
    }
    stats.nodes = w.node_count() as u64;
    stats.edges = w.edge_count() as u64;

    // Land cover.
    let ring = |refs: &[i64]| -> Option<LineString<f64>> {
        refs.iter()
            .map(|&id| {
                let [lat, lon] = pos[resolve(id)?];
                Some((lon as f64, lat as f64))
            })
            .collect::<Option<Vec<_>>>()
            .map(LineString::from)
    };
    let mut polygons: Vec<(Cover, Polygon<f64>)> = Vec::new();
    for (c, refs) in &cover_ways {
        match ring(refs) {
            Some(r) => polygons.push((*c, Polygon::new(r, vec![]))),
            None => stats.rings_failed += 1,
        }
    }
    for (c, members) in relations {
        let (mut outer, mut inner) = (Vec::new(), Vec::new());
        for (id, is_inner) in members {
            if let Some(r) = member_refs.get(&id) {
                if is_inner { &mut inner } else { &mut outer }.push(r.clone());
            }
        }
        let mut polys: Vec<Polygon<f64>> = Vec::new();
        for r in join_rings(outer, &mut stats.rings_failed) {
            match ring(&r) {
                Some(r) => polys.push(Polygon::new(r, vec![])),
                None => stats.rings_failed += 1,
            }
        }
        for r in join_rings(inner, &mut stats.rings_failed) {
            let Some(r) = ring(&r) else {
                stats.rings_failed += 1;
                continue;
            };
            let p = Point::from(r.0[0]);
            match polys.iter_mut().find(|o| o.intersects(&p)) {
                Some(o) => o.interiors_push(r),
                None => stats.rings_failed += 1,
            }
        }
        polygons.extend(polys.into_iter().map(|p| (c, p)));
    }
    let overlaps = |p: &Polygon<f64>| {
        bbox.is_none_or(|b| {
            p.bounding_rect().is_some_and(|r| {
                r.min().x <= b.max_lon
                    && r.max().x >= b.min_lon
                    && r.min().y <= b.max_lat
                    && r.max().y >= b.min_lat
            })
        })
    };
    for (c, p) in polygons.into_iter().filter(|(_, p)| overlaps(p)) {
        match c {
            Cover::Forest => extras.forest.push(p),
            Cover::Residential => extras.residential.push(p),
        }
    }

    Ok((w, stats, extras))
}

fn graph_node(w: &mut GraphWriter, gid: &mut [u32], pos: &[[f32; 2]], i: u32) -> u32 {
    let i = i as usize;
    if gid[i] == u32::MAX {
        let [lat, lon] = pos[i];
        let p = LatLon {
            lat: lat as f64,
            lon: lon as f64,
        };
        gid[i] = w.add_node(p, 0.0);
    }
    gid[i]
}

/// One edge over `seg` (node-table indices). A self-loop is split at its
/// middle vertex; its intermediate vertices are never split points, so the
/// halves always have distinct ends. A 2-vertex self-loop is dropped.
fn add_segment(
    w: &mut GraphWriter,
    gid: &mut [u32],
    pos: &[[f32; 2]],
    seg: &[u32],
    attrs: EdgeAttrs,
) {
    let last = seg.len() - 1;
    if seg[0] == seg[last] {
        if seg.len() >= 3 {
            let mid = seg.len() / 2;
            add_segment(w, gid, pos, &seg[..=mid], attrs);
            add_segment(w, gid, pos, &seg[mid..], attrs);
        }
        return;
    }
    let from = graph_node(w, gid, pos, seg[0]);
    let to = graph_node(w, gid, pos, seg[last]);
    let geom: Vec<(f64, f64, f32)> = seg
        .iter()
        .map(|&i| {
            let [lat, lon] = pos[i as usize];
            (lat as f64, lon as f64, 0.0)
        })
        .collect();
    let attrs = EdgeAttrs {
        length_m: polyline_length_m(geom.iter().copied()) as f32,
        ..attrs
    };
    w.add_edge(from, to, attrs, &geom);
}

/// Joins member ways end to end (either orientation) into closed rings of node
/// ids. A chain that cannot be closed counts in `failed` and is dropped.
fn join_rings(mut parts: Vec<Vec<i64>>, failed: &mut u64) -> Vec<Vec<i64>> {
    let mut rings = Vec::new();
    'next: while let Some(mut ring) = parts.pop() {
        if ring.len() < 2 {
            *failed += 1;
            continue;
        }
        while ring[0] != ring[ring.len() - 1] {
            let end = ring[ring.len() - 1];
            let Some(i) = parts
                .iter()
                .position(|p| p.first() == Some(&end) || p.last() == Some(&end))
            else {
                *failed += 1;
                continue 'next;
            };
            let mut p = parts.swap_remove(i);
            if p[0] != end {
                p.reverse();
            }
            ring.extend_from_slice(&p[1..]);
        }
        if ring.len() < 4 {
            *failed += 1;
            continue;
        }
        rings.push(ring);
    }
    rings
}
