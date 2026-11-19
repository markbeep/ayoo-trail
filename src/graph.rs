//! Graph file format (`docs/design.md` section 7): a single little-endian,
//! struct-of-arrays file read zero-copy from an mmap or an aligned buffer.
//!
//! [`layout`] is the one place that knows where every section lives; the
//! writer and the reader both use it.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::mem::size_of;
use std::path::Path;
use std::sync::OnceLock;

use crate::snap::{build_snap_index, SnapGrid};

use bytemuck::Pod;

#[cfg(target_endian = "big")]
compile_error!(
    "the graph file is little-endian and read zero-copy; big-endian hosts are unsupported"
);

const MAGIC: [u8; 4] = *b"TRGR";
const VERSION: u32 = 2;
const HEADER_LEN: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LatLon {
    pub lat: f64,
    pub lon: f64,
}

/// Direction of travel along an edge. `Forward` is `from -> to`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    Forward,
    Backward,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Surface {
    Asphalt = 0,
    Paved = 1,
    Compacted = 2,
    Gravel = 3,
    Dirt = 4,
    Grass = 5,
    Rock = 6,
    Unknown = 7,
}

impl Surface {
    /// Any value above 7 maps to `Unknown`.
    pub fn from_u8(v: u8) -> Surface {
        match v {
            0 => Surface::Asphalt,
            1 => Surface::Paved,
            2 => Surface::Compacted,
            3 => Surface::Gravel,
            4 => Surface::Dirt,
            5 => Surface::Grass,
            6 => Surface::Rock,
            _ => Surface::Unknown,
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Highway {
    Path = 0,
    Footway = 1,
    Bridleway = 2,
    Steps = 3,
    Track = 4,
    Cycleway = 5,
    Pedestrian = 6,
    LivingStreet = 7,
    Service = 8,
    Residential = 9,
    Unclassified = 10,
    Tertiary = 11,
    Secondary = 12,
    Primary = 13,
    Other = 255,
}

impl Highway {
    /// Unknown values map to `Other`.
    pub fn from_u8(v: u8) -> Highway {
        match v {
            0 => Highway::Path,
            1 => Highway::Footway,
            2 => Highway::Bridleway,
            3 => Highway::Steps,
            4 => Highway::Track,
            5 => Highway::Cycleway,
            6 => Highway::Pedestrian,
            7 => Highway::LivingStreet,
            8 => Highway::Service,
            9 => Highway::Residential,
            10 => Highway::Unclassified,
            11 => Highway::Tertiary,
            12 => Highway::Secondary,
            13 => Highway::Primary,
            _ => Highway::Other,
        }
    }
}

pub mod flags {
    pub const BRIDGE: u8 = 1;
    pub const TUNNEL: u8 = 2;
    pub const INFORMAL: u8 = 4;
    pub const MISSING_TAGS: u8 = 8;
    pub const FERRY: u8 = 16;
    pub const SURFACE_INFERRED: u8 = 32;
}

/// Nine signed grade bands, forward direction, as length shares summing to 255:
/// [<=-25, -25..-20, -20..-15, -15..-8, -8..8, 8..15, 15..20, 20..25, >=25] percent.
/// Backward traversal reads the array reversed.
pub const GRADE_BANDS: usize = 9;
pub const FLAT_BAND: usize = 4;
pub const FLAT_GRADE: [u8; GRADE_BANDS] = [0, 0, 0, 0, 255, 0, 0, 0, 0];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EdgeAttrs {
    pub length_m: f32,
    /// Meters, forward direction.
    pub gain: i16,
    pub loss: i16,
    pub grade: [u8; GRADE_BANDS],
    pub surface: Surface,
    /// 0 = missing, 1..=6 = T1..T6.
    pub sac: u8,
    pub highway: Highway,
    /// 0..=100.
    pub forest_pct: u8,
    /// 0 none, 1 within 200 m, 2 inside.
    pub urban: u8,
    /// 0 none, 1..=3.
    pub official_category: u8,
    pub flags: u8,
}

impl Default for EdgeAttrs {
    fn default() -> Self {
        EdgeAttrs {
            length_m: 0.0,
            gain: 0,
            loss: 0,
            grade: FLAT_GRADE,
            surface: Surface::Unknown,
            sac: 0,
            highway: Highway::Path,
            forest_pct: 0,
            urban: 0,
            official_category: 0,
            flags: 0,
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnchorKind {
    Peak = 0,
    Viewpoint = 1,
    Other = 255,
}

impl AnchorKind {
    fn from_u8(v: u8) -> AnchorKind {
        match v {
            0 => AnchorKind::Peak,
            1 => AnchorKind::Viewpoint,
            _ => AnchorKind::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Anchor {
    pub pos: LatLon,
    pub kind: AnchorKind,
    pub weight: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct Bbox {
    pub min_lat: f64,
    pub min_lon: f64,
    pub max_lat: f64,
    pub max_lon: f64,
}

#[derive(Debug, thiserror::Error)]
pub enum GraphError {
    #[error("graph file I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("not a trail-calc graph file (bad magic)")]
    Magic,
    #[error("unsupported graph file version {0}")]
    Version(u32),
    #[error("graph file is truncated")]
    Truncated,
    #[error("graph file contains an out-of-range index")]
    Index,
}

// ---------------------------------------------------------------- layout

#[derive(Debug, Clone, Copy)]
struct Counts {
    nodes: usize,
    edges: usize,
    geom: usize,
    anchors: usize,
    /// Snap grid cells, derived from the bbox (`SnapGrid::cells`).
    cells: usize,
    /// Snap index entries (header bytes 52..56).
    snap: usize,
}

/// Byte offset of every section, plus the total file size.
#[derive(Debug, Clone, Copy)]
struct Layout {
    node_lat: usize,
    node_lon: usize,
    node_elev: usize,
    node_first: usize,
    adj: usize,
    edge_from: usize,
    edge_to: usize,
    edge_len_cm: usize,
    edge_gain: usize,
    edge_loss: usize,
    edge_grade: usize,
    edge_surface: usize,
    edge_sac: usize,
    edge_highway: usize,
    edge_forest: usize,
    edge_urban: usize,
    edge_official: usize,
    edge_flags: usize,
    edge_geom_start: usize,
    edge_geom_len: usize,
    geom_lat: usize,
    geom_lon: usize,
    geom_z: usize,
    anchor_lat: usize,
    anchor_lon: usize,
    anchor_kind: usize,
    anchor_weight: usize,
    snap_first: usize,
    snap_seg: usize,
    total: usize,
}

fn layout(c: Counts) -> Layout {
    let mut pos = HEADER_LEN;
    // Each section starts 8-byte aligned; `total` is the end of the last one.
    let mut sec = |elem: usize, count: usize| {
        pos = pos.next_multiple_of(8);
        let start = pos;
        pos += elem * count;
        start
    };
    let (n, m, g, a) = (c.nodes, c.edges, c.geom, c.anchors);
    let node_lat = sec(4, n);
    let node_lon = sec(4, n);
    let node_elev = sec(2, n);
    let node_first = sec(4, n + 1);
    let adj = sec(4, 2 * m);
    let edge_from = sec(4, m);
    let edge_to = sec(4, m);
    let edge_len_cm = sec(4, m);
    let edge_gain = sec(2, m);
    let edge_loss = sec(2, m);
    let edge_grade = sec(1, GRADE_BANDS * m);
    let edge_surface = sec(1, m);
    let edge_sac = sec(1, m);
    let edge_highway = sec(1, m);
    let edge_forest = sec(1, m);
    let edge_urban = sec(1, m);
    let edge_official = sec(1, m);
    let edge_flags = sec(1, m);
    let edge_geom_start = sec(4, m);
    let edge_geom_len = sec(4, m);
    let geom_lat = sec(4, g);
    let geom_lon = sec(4, g);
    let geom_z = sec(2, g);
    let anchor_lat = sec(4, a);
    let anchor_lon = sec(4, a);
    let anchor_kind = sec(1, a);
    let anchor_weight = sec(4, a);
    let snap_first = sec(4, c.cells + 1);
    let snap_seg = sec(4, c.snap);
    let total = pos;
    Layout {
        node_lat,
        node_lon,
        node_elev,
        node_first,
        adj,
        edge_from,
        edge_to,
        edge_len_cm,
        edge_gain,
        edge_loss,
        edge_grade,
        edge_surface,
        edge_sac,
        edge_highway,
        edge_forest,
        edge_urban,
        edge_official,
        edge_flags,
        edge_geom_start,
        edge_geom_len,
        geom_lat,
        geom_lon,
        geom_z,
        anchor_lat,
        anchor_lon,
        anchor_kind,
        anchor_weight,
        snap_first,
        snap_seg,
        total,
    }
}

// ---------------------------------------------------------------- writer

struct StagedEdge {
    from: u32,
    to: u32,
    attrs: EdgeAttrs,
    geometry: Vec<(f64, f64, f32)>,
}

/// Staged graph builder. Geometry stays per edge so later passes can rewrite
/// it; everything is flattened into the SoA file by `write_to` / `finish`.
pub struct GraphWriter {
    osm_date: i64,
    dem_source: u8,
    nodes: Vec<(LatLon, f32)>,
    edges: Vec<StagedEdge>,
    anchors: Vec<Anchor>,
}

fn round_i16(v: f32) -> i16 {
    v.round().clamp(i16::MIN as f32, i16::MAX as f32) as i16
}

impl GraphWriter {
    pub fn new(osm_date: i64) -> Self {
        GraphWriter {
            osm_date,
            dem_source: 0,
            nodes: Vec::new(),
            edges: Vec::new(),
            anchors: Vec::new(),
        }
    }

    pub fn add_node(&mut self, pos: LatLon, elev_m: f32) -> u32 {
        self.nodes.push((pos, elev_m));
        (self.nodes.len() - 1) as u32
    }

    /// `geometry` = (lat, lon, z) including both endpoints. Returns the edge id.
    /// Panics if `from` or `to` is not a node added to this writer.
    pub fn add_edge(
        &mut self,
        from: u32,
        to: u32,
        attrs: EdgeAttrs,
        geometry: &[(f64, f64, f32)],
    ) -> u32 {
        assert!(
            (from as usize) < self.nodes.len() && (to as usize) < self.nodes.len(),
            "add_edge: node id out of range"
        );
        self.edges.push(StagedEdge {
            from,
            to,
            attrs,
            geometry: geometry.to_vec(),
        });
        (self.edges.len() - 1) as u32
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    pub fn node_pos(&self, n: u32) -> LatLon {
        self.nodes[n as usize].0
    }

    pub fn set_node_elev(&mut self, n: u32, elev_m: f32) {
        self.nodes[n as usize].1 = elev_m;
    }

    pub fn edge_endpoints(&self, e: u32) -> (u32, u32) {
        let e = &self.edges[e as usize];
        (e.from, e.to)
    }

    pub fn edge_attrs(&self, e: u32) -> &EdgeAttrs {
        &self.edges[e as usize].attrs
    }

    pub fn edge_attrs_mut(&mut self, e: u32) -> &mut EdgeAttrs {
        &mut self.edges[e as usize].attrs
    }

    pub fn edge_geometry(&self, e: u32) -> &[(f64, f64, f32)] {
        &self.edges[e as usize].geometry
    }

    pub fn set_edge_geometry(&mut self, e: u32, geometry: Vec<(f64, f64, f32)>) {
        self.edges[e as usize].geometry = geometry;
    }

    pub fn add_anchor(&mut self, pos: LatLon, kind: AnchorKind, weight: f32) {
        self.anchors.push(Anchor { pos, kind, weight });
    }

    /// 0 none, 1 swissALTI3D, 2 DHM25.
    pub fn set_dem_source(&mut self, id: u8) {
        self.dem_source = id;
    }

    /// Streams the sections through a `BufWriter`.
    pub fn write_to(&self, path: &Path) -> Result<(), GraphError> {
        let mut out = BufWriter::new(File::create(path)?);
        self.write_sections(&mut out)?;
        out.flush()?;
        Ok(())
    }

    /// Same bytes as `write_to`.
    pub fn finish(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        self.write_sections(&mut buf)
            .expect("writing to a Vec cannot fail");
        buf
    }

    fn counts(&self) -> Counts {
        Counts {
            nodes: self.nodes.len(),
            edges: self.edges.len(),
            geom: self.edges.iter().map(|e| e.geometry.len()).sum(),
            anchors: self.anchors.len(),
            cells: 0,
            snap: 0,
        }
    }

    fn write_sections<W: Write>(&self, w: &mut W) -> std::io::Result<()> {
        let mut counts = self.counts();
        let (n, m) = (counts.nodes, counts.edges);

        // Bbox over geometry vertices (or nodes if there are none), computed on
        // the f32 values actually stored so it encloses them.
        let mut pts: Box<dyn Iterator<Item = (f32, f32)> + '_> = Box::new(
            self.edges
                .iter()
                .flat_map(|e| e.geometry.iter().map(|g| (g.0 as f32, g.1 as f32))),
        );
        if counts.geom == 0 {
            pts = Box::new(self.nodes.iter().map(|(p, _)| (p.lat as f32, p.lon as f32)));
        }
        let mut bbox = [
            f32::INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
        ];
        for (lat, lon) in pts {
            bbox[0] = bbox[0].min(lat);
            bbox[1] = bbox[1].min(lon);
            bbox[2] = bbox[2].max(lat);
            bbox[3] = bbox[3].max(lon);
        }
        if bbox[0] > bbox[2] {
            bbox = [0.0; 4];
        }
        let grid = SnapGrid::new(Bbox {
            min_lat: bbox[0] as f64,
            min_lon: bbox[1] as f64,
            max_lat: bbox[2] as f64,
            max_lon: bbox[3] as f64,
        });
        let (snap_first, snap_seg) =
            build_snap_index(&grid, self.edges.iter().map(|e| e.geometry.as_slice()));
        counts.cells = grid.cells();
        counts.snap = snap_seg.len();
        let l = layout(counts);

        let mut header = [0u8; HEADER_LEN];
        header[0..4].copy_from_slice(&MAGIC);
        header[4..8].copy_from_slice(&VERSION.to_le_bytes());
        header[8..16].copy_from_slice(&self.osm_date.to_le_bytes());
        header[16..20].copy_from_slice(&(n as u32).to_le_bytes());
        header[20..24].copy_from_slice(&(m as u32).to_le_bytes());
        header[24..28].copy_from_slice(&(counts.geom as u32).to_le_bytes());
        header[28..32].copy_from_slice(&(counts.anchors as u32).to_le_bytes());
        for (i, v) in bbox.iter().enumerate() {
            header[32 + 4 * i..36 + 4 * i].copy_from_slice(&v.to_le_bytes());
        }
        header[48] = self.dem_source;
        header[52..56].copy_from_slice(&(counts.snap as u32).to_le_bytes());

        let mut out = SectionWriter { w, pos: 0 };
        out.put(0, &header)?;

        let lat: Vec<f32> = self.nodes.iter().map(|x| x.0.lat as f32).collect();
        out.put(l.node_lat, bytemuck::cast_slice(&lat))?;
        let lon: Vec<f32> = self.nodes.iter().map(|x| x.0.lon as f32).collect();
        out.put(l.node_lon, bytemuck::cast_slice(&lon))?;
        let elev: Vec<i16> = self.nodes.iter().map(|x| round_i16(x.1)).collect();
        out.put(l.node_elev, bytemuck::cast_slice(&elev))?;

        // CSR: each edge appears at `from` (forward) and `to` (backward).
        let mut first = vec![0u32; n + 1];
        for e in &self.edges {
            first[e.from as usize + 1] += 1;
            first[e.to as usize + 1] += 1;
        }
        for i in 0..n {
            first[i + 1] += first[i];
        }
        let mut cursor = first.clone();
        let mut adj = vec![0u32; 2 * m];
        for (i, e) in self.edges.iter().enumerate() {
            let c = &mut cursor[e.from as usize];
            adj[*c as usize] = (i as u32) << 1;
            *c += 1;
            let c = &mut cursor[e.to as usize];
            adj[*c as usize] = ((i as u32) << 1) | 1;
            *c += 1;
        }
        out.put(l.node_first, bytemuck::cast_slice(&first))?;
        out.put(l.adj, bytemuck::cast_slice(&adj))?;

        let col = |f: fn(&StagedEdge) -> u32| -> Vec<u32> { self.edges.iter().map(f).collect() };
        out.put(l.edge_from, bytemuck::cast_slice(&col(|e| e.from)))?;
        out.put(l.edge_to, bytemuck::cast_slice(&col(|e| e.to)))?;
        out.put(
            l.edge_len_cm,
            bytemuck::cast_slice(&col(|e| (e.attrs.length_m * 100.0).round() as u32)),
        )?;
        let gain: Vec<i16> = self.edges.iter().map(|e| e.attrs.gain).collect();
        out.put(l.edge_gain, bytemuck::cast_slice(&gain))?;
        let loss: Vec<i16> = self.edges.iter().map(|e| e.attrs.loss).collect();
        out.put(l.edge_loss, bytemuck::cast_slice(&loss))?;
        let grade: Vec<u8> = self.edges.iter().flat_map(|e| e.attrs.grade).collect();
        out.put(l.edge_grade, &grade)?;
        let bytes = |f: fn(&EdgeAttrs) -> u8| -> Vec<u8> {
            self.edges.iter().map(|e| f(&e.attrs)).collect()
        };
        out.put(l.edge_surface, &bytes(|a| a.surface as u8))?;
        out.put(l.edge_sac, &bytes(|a| a.sac))?;
        out.put(l.edge_highway, &bytes(|a| a.highway as u8))?;
        out.put(l.edge_forest, &bytes(|a| a.forest_pct))?;
        out.put(l.edge_urban, &bytes(|a| a.urban))?;
        out.put(l.edge_official, &bytes(|a| a.official_category))?;
        out.put(l.edge_flags, &bytes(|a| a.flags))?;

        let mut start = 0u32;
        let mut geom_start = Vec::with_capacity(m);
        let mut geom_len = Vec::with_capacity(m);
        for e in &self.edges {
            geom_start.push(start);
            geom_len.push(e.geometry.len() as u32);
            start += e.geometry.len() as u32;
        }
        out.put(l.edge_geom_start, bytemuck::cast_slice(&geom_start))?;
        out.put(l.edge_geom_len, bytemuck::cast_slice(&geom_len))?;

        let verts = || self.edges.iter().flat_map(|e| e.geometry.iter());
        let glat: Vec<f32> = verts().map(|v| v.0 as f32).collect();
        out.put(l.geom_lat, bytemuck::cast_slice(&glat))?;
        let glon: Vec<f32> = verts().map(|v| v.1 as f32).collect();
        out.put(l.geom_lon, bytemuck::cast_slice(&glon))?;
        let gz: Vec<i16> = verts().map(|v| round_i16(v.2)).collect();
        out.put(l.geom_z, bytemuck::cast_slice(&gz))?;

        let alat: Vec<f32> = self.anchors.iter().map(|a| a.pos.lat as f32).collect();
        out.put(l.anchor_lat, bytemuck::cast_slice(&alat))?;
        let alon: Vec<f32> = self.anchors.iter().map(|a| a.pos.lon as f32).collect();
        out.put(l.anchor_lon, bytemuck::cast_slice(&alon))?;
        let akind: Vec<u8> = self.anchors.iter().map(|a| a.kind as u8).collect();
        out.put(l.anchor_kind, &akind)?;
        let aw: Vec<f32> = self.anchors.iter().map(|a| a.weight).collect();
        out.put(l.anchor_weight, bytemuck::cast_slice(&aw))?;
        out.put(l.snap_first, bytemuck::cast_slice(&snap_first))?;
        out.put(l.snap_seg, bytemuck::cast_slice(&snap_seg))?;

        // Pad up to the layout's total so empty trailing sections still count.
        out.put(l.total, &[])
    }
}

/// Writes byte runs at increasing offsets, zero-filling the gaps.
struct SectionWriter<'a, W: Write> {
    w: &'a mut W,
    pos: usize,
}

impl<W: Write> SectionWriter<'_, W> {
    fn put(&mut self, offset: usize, data: &[u8]) -> std::io::Result<()> {
        const ZEROS: [u8; 8] = [0; 8];
        debug_assert!(offset >= self.pos);
        while self.pos < offset {
            let k = (offset - self.pos).min(ZEROS.len());
            self.w.write_all(&ZEROS[..k])?;
            self.pos += k;
        }
        self.w.write_all(data)?;
        self.pos += data.len();
        Ok(())
    }
}

// ---------------------------------------------------------------- reader

enum Backing {
    /// `u64` storage guarantees 8-byte alignment; `len` is the byte length.
    Owned(Vec<u64>, usize),
    Mapped(memmap2::Mmap),
}

impl Backing {
    fn bytes(&self) -> &[u8] {
        match self {
            Backing::Owned(v, len) => &bytemuck::cast_slice::<u64, u8>(v)[..*len],
            Backing::Mapped(m) => m,
        }
    }
}

pub struct Graph {
    backing: Backing,
    layout: Layout,
    counts: Counts,
    osm_date: i64,
    bbox: Bbox,
    dem_source: u8,
    anchors: OnceLock<Vec<Anchor>>,
    /// Connected-component id per node and the id of the largest component.
    components: OnceLock<(Vec<u32>, u32)>,
    snap_grid: SnapGrid,
}

fn le_u32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().expect("4 bytes"))
}

fn le_f32(b: &[u8], at: usize) -> f32 {
    f32::from_le_bytes(b[at..at + 4].try_into().expect("4 bytes"))
}

impl Graph {
    /// Copies `bytes` into an 8-aligned `Vec<u64>` backing.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Graph, GraphError> {
        let mut words = vec![0u64; bytes.len().div_ceil(8)];
        bytemuck::cast_slice_mut::<u64, u8>(&mut words)[..bytes.len()].copy_from_slice(&bytes);
        Graph::from_backing(Backing::Owned(words, bytes.len()))
    }

    /// Memory-maps the file.
    pub fn open(path: &Path) -> Result<Graph, GraphError> {
        let file = File::open(path)?;
        // SAFETY: the file is opened read-only; like any mmap-backed reader we
        // rely on nobody truncating it while the graph is in use.
        let map = unsafe { memmap2::Mmap::map(&file)? };
        Graph::from_backing(Backing::Mapped(map))
    }

    fn from_backing(backing: Backing) -> Result<Graph, GraphError> {
        let b = backing.bytes();
        if b.len() < HEADER_LEN {
            return Err(GraphError::Truncated);
        }
        if b[0..4] != MAGIC {
            return Err(GraphError::Magic);
        }
        let version = le_u32(b, 4);
        if version != VERSION {
            return Err(GraphError::Version(version));
        }
        let osm_date = i64::from_le_bytes(b[8..16].try_into().expect("8 bytes"));
        let counts = Counts {
            nodes: le_u32(b, 16) as usize,
            edges: le_u32(b, 20) as usize,
            geom: le_u32(b, 24) as usize,
            anchors: le_u32(b, 28) as usize,
            cells: 0,
            snap: le_u32(b, 52) as usize,
        };
        let bbox = Bbox {
            min_lat: le_f32(b, 32) as f64,
            min_lon: le_f32(b, 36) as f64,
            max_lat: le_f32(b, 40) as f64,
            max_lon: le_f32(b, 44) as f64,
        };
        let dem_source = b[48];
        let snap_grid = SnapGrid::new(bbox);
        let counts = Counts {
            cells: snap_grid.cells(),
            ..counts
        };
        let layout = layout(counts);
        if b.len() < layout.total {
            return Err(GraphError::Truncated);
        }
        let g = Graph {
            backing,
            layout,
            counts,
            osm_date,
            bbox,
            dem_source,
            anchors: OnceLock::new(),
            components: OnceLock::new(),
            snap_grid,
        };
        g.validate()?;
        Ok(g)
    }

    /// Checks every stored index once so accessors can index freely.
    fn validate(&self) -> Result<(), GraphError> {
        let (n, m, g) = (self.counts.nodes, self.counts.edges, self.counts.geom);
        let first = self.node_first();
        if first[0] != 0 || first[n] as usize != 2 * m || first.windows(2).any(|w| w[0] > w[1]) {
            return Err(GraphError::Index);
        }
        if self.adj().iter().any(|&v| (v >> 1) as usize >= m) {
            return Err(GraphError::Index);
        }
        let from = self.col::<u32>(self.layout.edge_from, m);
        let to = self.col::<u32>(self.layout.edge_to, m);
        if from.iter().chain(to).any(|&v| v as usize >= n) {
            return Err(GraphError::Index);
        }
        let gs = self.col::<u32>(self.layout.edge_geom_start, m);
        let gl = self.col::<u32>(self.layout.edge_geom_len, m);
        if gs
            .iter()
            .zip(gl)
            .any(|(&s, &l)| s as u64 + l as u64 > g as u64)
        {
            return Err(GraphError::Index);
        }
        // `geom_edge` binary-searches geom_start; snap entries name a segment start.
        if gs.first().is_some_and(|&s| s != 0) || gs.windows(2).any(|w| w[0] > w[1]) {
            return Err(GraphError::Index);
        }
        let (first, segs) = self.snap_cells();
        if first[0] != 0
            || first[self.counts.cells] as usize != self.counts.snap
            || first.windows(2).any(|w| w[0] > w[1])
            || segs.iter().any(|&s| s as u64 + 1 >= g as u64)
            || (m == 0 && !segs.is_empty())
        {
            return Err(GraphError::Index);
        }
        Ok(())
    }

    pub(crate) fn snap_grid(&self) -> &SnapGrid {
        &self.snap_grid
    }

    /// (`first[cells + 1]`, `segs`) of the snap index, see `build_snap_index`.
    pub(crate) fn snap_cells(&self) -> (&[u32], &[u32]) {
        let l = &self.layout;
        (
            self.col(l.snap_first, self.counts.cells + 1),
            self.col(l.snap_seg, self.counts.snap),
        )
    }

    /// Projected position of global geometry vertex `i`.
    pub(crate) fn geom_xy(&self, i: u32) -> [f64; 2] {
        let (l, g) = (&self.layout, self.counts.geom);
        self.snap_grid.xy(
            self.col::<f32>(l.geom_lat, g)[i as usize],
            self.col::<f32>(l.geom_lon, g)[i as usize],
        )
    }

    /// (edge, segment index within it) of the segment starting at global vertex `i`.
    pub(crate) fn geom_edge(&self, i: u32) -> (u32, u32) {
        let gs = self.col::<u32>(self.layout.edge_geom_start, self.counts.edges);
        let e = gs.partition_point(|&s| s <= i) - 1;
        (e as u32, i - gs[e])
    }

    /// Typed view of a section. Offsets are 8-aligned and bounds were checked
    /// against `layout.total` at load, so this cannot fail on a loaded graph.
    fn col<T: Pod>(&self, offset: usize, count: usize) -> &[T] {
        bytemuck::cast_slice(&self.backing.bytes()[offset..offset + count * size_of::<T>()])
    }

    fn node_first(&self) -> &[u32] {
        self.col(self.layout.node_first, self.counts.nodes + 1)
    }

    fn adj(&self) -> &[u32] {
        self.col(self.layout.adj, 2 * self.counts.edges)
    }

    fn edge_col<T: Pod>(&self, offset: usize, e: u32) -> T {
        self.col::<T>(offset, self.counts.edges)[e as usize]
    }

    pub fn node_count(&self) -> usize {
        self.counts.nodes
    }

    pub fn edge_count(&self) -> usize {
        self.counts.edges
    }

    pub fn osm_date(&self) -> i64 {
        self.osm_date
    }

    pub fn bbox(&self) -> Bbox {
        self.bbox
    }

    pub fn dem_source(&self) -> u8 {
        self.dem_source
    }

    pub fn node(&self, n: u32) -> LatLon {
        let c = self.counts.nodes;
        LatLon {
            lat: self.col::<f32>(self.layout.node_lat, c)[n as usize] as f64,
            lon: self.col::<f32>(self.layout.node_lon, c)[n as usize] as f64,
        }
    }

    pub fn node_elev(&self, n: u32) -> f32 {
        self.col::<i16>(self.layout.node_elev, self.counts.nodes)[n as usize] as f32
    }

    pub fn edge_from(&self, e: u32) -> u32 {
        self.edge_col(self.layout.edge_from, e)
    }

    pub fn edge_to(&self, e: u32) -> u32 {
        self.edge_col(self.layout.edge_to, e)
    }

    pub fn edge_len_m(&self, e: u32) -> f64 {
        self.edge_col::<u32>(self.layout.edge_len_cm, e) as f64 / 100.0
    }

    /// Copy assembled from the SoA columns.
    pub fn edge_attrs(&self, e: u32) -> EdgeAttrs {
        let l = &self.layout;
        let grade = self.col::<u8>(l.edge_grade, GRADE_BANDS * self.counts.edges);
        let i = e as usize * GRADE_BANDS;
        EdgeAttrs {
            length_m: (self.edge_col::<u32>(l.edge_len_cm, e) as f64 / 100.0) as f32,
            gain: self.edge_col(l.edge_gain, e),
            loss: self.edge_col(l.edge_loss, e),
            grade: grade[i..i + GRADE_BANDS].try_into().expect("9 bands"),
            surface: Surface::from_u8(self.edge_col(l.edge_surface, e)),
            sac: self.edge_col(l.edge_sac, e),
            highway: Highway::from_u8(self.edge_col(l.edge_highway, e)),
            forest_pct: self.edge_col(l.edge_forest, e),
            urban: self.edge_col(l.edge_urban, e),
            official_category: self.edge_col(l.edge_official, e),
            flags: self.edge_col(l.edge_flags, e),
        }
    }

    pub fn edge_geometry(
        &self,
        e: u32,
    ) -> impl ExactSizeIterator<Item = (f64, f64, f32)> + DoubleEndedIterator + '_ {
        let l = &self.layout;
        let start = self.edge_col::<u32>(l.edge_geom_start, e) as usize;
        let len = self.edge_col::<u32>(l.edge_geom_len, e) as usize;
        let g = self.counts.geom;
        let lat = &self.col::<f32>(l.geom_lat, g)[start..start + len];
        let lon = &self.col::<f32>(l.geom_lon, g)[start..start + len];
        let z = &self.col::<i16>(l.geom_z, g)[start..start + len];
        lat.iter()
            .zip(lon)
            .zip(z)
            .map(|((&la, &lo), &z)| (la as f64, lo as f64, z as f32))
    }

    /// Edges incident to `n`, with the direction in which leaving `n` traverses them.
    pub fn neighbors(&self, n: u32) -> impl Iterator<Item = (u32, Direction)> + '_ {
        let first = self.node_first();
        let range = first[n as usize] as usize..first[n as usize + 1] as usize;
        self.adj()[range].iter().map(|&v| {
            let dir = if v & 1 == 1 {
                Direction::Backward
            } else {
                Direction::Forward
            };
            (v >> 1, dir)
        })
    }

    /// Connected component of an edge, ignoring profiles. Islands (station stairs, courtyards)
    /// can't be reached from the main network, and A* only gives up on one after exploring
    /// everything reachable, so snaps stay inside the start's component.
    pub fn edge_component(&self, e: u32) -> u32 {
        self.components().0[self.edge_from(e) as usize]
    }

    /// The largest connected component (the main network).
    pub fn main_component(&self) -> u32 {
        self.components().1
    }

    /// Union-find over all edges, once (~0.1 s on the CH graph).
    fn components(&self) -> &(Vec<u32>, u32) {
        self.components.get_or_init(|| {
            fn find(p: &mut [u32], mut x: u32) -> u32 {
                while p[x as usize] != x {
                    p[x as usize] = p[p[x as usize] as usize];
                    x = p[x as usize];
                }
                x
            }
            let n = self.node_count();
            let mut parent: Vec<u32> = (0..n as u32).collect();
            for e in 0..self.edge_count() as u32 {
                let (a, b) = (
                    find(&mut parent, self.edge_from(e)),
                    find(&mut parent, self.edge_to(e)),
                );
                if a != b {
                    parent[a as usize] = b;
                }
            }
            let mut size = vec![0u32; n];
            for v in 0..n as u32 {
                let r = find(&mut parent, v);
                parent[v as usize] = r;
                size[r as usize] += 1;
            }
            let main = (0..n as u32).max_by_key(|&v| size[v as usize]).unwrap_or(0);
            (parent, main)
        })
    }

    /// Decoded once, then cached.
    pub fn anchors(&self) -> &[Anchor] {
        self.anchors.get_or_init(|| {
            let (l, a) = (&self.layout, self.counts.anchors);
            let lat = self.col::<f32>(l.anchor_lat, a);
            let lon = self.col::<f32>(l.anchor_lon, a);
            let kind = self.col::<u8>(l.anchor_kind, a);
            let weight = self.col::<f32>(l.anchor_weight, a);
            (0..a)
                .map(|i| Anchor {
                    pos: LatLon {
                        lat: lat[i] as f64,
                        lon: lon[i] as f64,
                    },
                    kind: AnchorKind::from_u8(kind[i]),
                    weight: weight[i],
                })
                .collect()
        })
    }
}
