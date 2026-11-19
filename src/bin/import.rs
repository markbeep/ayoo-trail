//! `trail-calc-import`: OSM → DEM → official trails → land cover → anchors → `graph.bin`.

use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Instant, UNIX_EPOCH};

use clap::{Parser, ValueEnum};
use trail_calc::graph::{flags, AnchorKind, Graph, Surface, GRADE_BANDS};
use trail_calc::import::alti3d::Alti3dTiles;
use trail_calc::import::ch1903::wgs84_to_lv95;
use trail_calc::import::dem::{apply_dem, AscGrid, DemStats};
use trail_calc::import::landcover::{apply_landcover, collect_anchors};
use trail_calc::import::official::{apply_official, read_official_gpkg, OFFICIAL_TOLERANCE_M};
use trail_calc::import::osm::import_osm;
use trail_calc::import::region;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Clone, Copy, ValueEnum)]
enum DemSource {
    Alti3d,
    Asc,
}

/// Paths default to `$DATA_DIR` (default `./data`).
#[derive(Parser)]
struct Args {
    #[arg(long, env = "REGION", default_value = "demo", value_parser = ["demo", "ch"])]
    region: String,
    /// Default `$DATA_DIR/raw/switzerland-latest.osm.pbf`.
    #[arg(long)]
    osm: Option<PathBuf>,
    #[arg(long, value_enum, default_value = "alti3d")]
    dem_source: DemSource,
    /// Default `$DATA_DIR/raw/alti3d` for alti3d; required for asc.
    #[arg(long)]
    dem: Option<PathBuf>,
    /// Default `$DATA_DIR/raw/wanderwege/SWISSTLM3D_WANDERWEGE.gpkg`.
    #[arg(long)]
    official: Option<PathBuf>,
    /// Default `$DATA_DIR/graph.bin`.
    #[arg(long)]
    out: Option<PathBuf>,
    /// Rebuild even if `--out` exists.
    #[arg(long)]
    force: bool,
}

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();
    match run(Args::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("trail-calc-import: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: Args) -> Result<(), Box<dyn Error>> {
    let data = std::env::var_os("DATA_DIR").map_or_else(|| PathBuf::from("data"), PathBuf::from);
    let out = args.out.unwrap_or_else(|| data.join("graph.bin"));
    if out.exists() && !args.force {
        println!(
            "{} exists, skipping import (use --force to rebuild)",
            out.display()
        );
        return Ok(());
    }
    let osm = args
        .osm
        .unwrap_or_else(|| data.join("raw/switzerland-latest.osm.pbf"));
    let dem = match (args.dem, args.dem_source) {
        (Some(p), _) => p,
        (None, DemSource::Alti3d) => data.join("raw/alti3d"),
        (None, DemSource::Asc) => return Err("--dem is required with --dem-source asc".into()),
    };
    let official = args
        .official
        .unwrap_or_else(|| data.join("raw/wanderwege/SWISSTLM3D_WANDERWEGE.gpkg"));
    for p in [&osm, &dem, &official] {
        if !p.exists() {
            return Err(format!("input not found: {}", p.display()).into());
        }
    }
    let region = region(&args.region).ok_or("unknown region")?;
    let start = Instant::now();

    // shortcut: the PBF's mtime stands in for the OSM extract date; read the PBF header timestamp if it matters.
    let osm_date = std::fs::metadata(&osm)?
        .modified()?
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    let t = Instant::now();
    let (mut w, osm_stats, extras) = import_osm(&osm, Some(region.bbox), osm_date)?;
    info!(?osm_stats, elapsed = ?t.elapsed(), "osm");

    let t = Instant::now();
    let dem_stats = match args.dem_source {
        DemSource::Alti3d => {
            let tiles = Alti3dTiles::open(&dem)?;
            let s = apply_dem(&mut w, &tiles);
            info!(
                tiles = tiles.tile_count(),
                decodes = tiles.decode_count(),
                "alti3d"
            );
            s
        }
        DemSource::Asc => apply_dem(&mut w, &AscGrid::from_asc(&dem)?),
    };
    info!(?dem_stats, elapsed = ?t.elapsed(), "dem");

    let t = Instant::now();
    let b = region.bbox;
    // All four corners: the WGS84 bbox is not axis-aligned in LV95. Padded 1 km.
    let corners = [
        (b.min_lat, b.min_lon),
        (b.min_lat, b.max_lon),
        (b.max_lat, b.min_lon),
        (b.max_lat, b.max_lon),
    ]
    .map(|(lat, lon)| wgs84_to_lv95(lat, lon));
    let mut clip = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for (e, n) in corners {
        clip = [
            clip[0].min(e - 1000.0),
            clip[1].min(n - 1000.0),
            clip[2].max(e + 1000.0),
            clip[3].max(n + 1000.0),
        ];
    }
    let lines = read_official_gpkg(&official, Some(clip))?;
    let marked = apply_official(&mut w, &lines, OFFICIAL_TOLERANCE_M);
    info!(lines = lines.len(), marked, elapsed = ?t.elapsed(), "official");

    let t = Instant::now();
    apply_landcover(&mut w, &extras);
    info!(forest = extras.forest.len(), residential = extras.residential.len(), elapsed = ?t.elapsed(), "landcover");
    collect_anchors(&mut w, &extras);

    let t = Instant::now();
    let mut tmp = out.clone().into_os_string();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    w.write_to(&tmp)?;
    std::fs::rename(&tmp, &out)?;
    info!(out = %out.display(), elapsed = ?t.elapsed(), total = ?start.elapsed(), "written");

    print_summary(&out, &dem_stats)?;
    Ok(())
}

/// Read back from the written file, so the summary also proves it loads.
fn print_summary(out: &Path, dem: &DemStats) -> Result<(), Box<dyn Error>> {
    let g = Graph::open(out)?;
    let km = |m: f64| m / 1000.0;
    let (mut total, mut missing, mut forest_m) = (0.0, 0.0, 0.0);
    let mut surface = [(0.0, 0.0); 8];
    let (mut sac, mut official, mut urban) = ([0.0; 7], [0.0; 4], [0.0; 3]);
    let mut grade = [0.0; GRADE_BANDS];
    for e in 0..g.edge_count() as u32 {
        let a = g.edge_attrs(e);
        let len = a.length_m as f64;
        total += len;
        let s = &mut surface[a.surface as usize];
        s.0 += len;
        if a.flags & flags::SURFACE_INFERRED != 0 {
            s.1 += len;
        }
        sac[(a.sac as usize).min(6)] += len;
        if a.flags & flags::MISSING_TAGS != 0 {
            missing += len;
        }
        official[(a.official_category as usize).min(3)] += len;
        forest_m += len * a.forest_pct as f64;
        urban[(a.urban as usize).min(2)] += len;
        for (b, &share) in a.grade.iter().enumerate() {
            grade[b] += len * share as f64 / 255.0;
        }
    }
    let share = |m: f64| if total > 0.0 { m / total * 100.0 } else { 0.0 };
    println!("import summary");
    println!(
        "  nodes {}, edges {}, {:.1} km",
        g.node_count(),
        g.edge_count(),
        km(total)
    );
    for (i, (m, inferred)) in surface.iter().enumerate() {
        println!(
            "  surface {:?}: {:.1} km ({:.1} km inferred)",
            Surface::from_u8(i as u8),
            km(*m),
            km(*inferred)
        );
    }
    for (i, m) in sac.iter().enumerate() {
        let name = if i == 0 {
            "missing".to_string()
        } else {
            format!("T{i}")
        };
        println!("  sac {name}: {:.1} km", km(*m));
    }
    println!(
        "  missing tags: {:.1} km ({:.1} %)",
        km(missing),
        share(missing)
    );
    let off: f64 = official[1..].iter().sum();
    println!("  official: {:.1} km ({:.1} %)", km(off), share(off));
    for (c, m) in official.iter().enumerate().skip(1) {
        println!("    category {c}: {:.1} km ({:.1} %)", km(*m), share(*m));
    }
    println!(
        "  forest: {:.1} % (length-weighted mean)",
        if total > 0.0 { forest_m / total } else { 0.0 }
    );
    for (c, m) in urban.iter().enumerate() {
        println!("  urban {c}: {:.1} km", km(*m));
    }
    let bands = [
        "<-25", "-25..-20", "-20..-15", "-15..-8", "-8..8", "8..15", "15..20", "20..25", ">25",
    ];
    for (b, m) in bands.iter().zip(grade) {
        println!("  grade {b} %: {:.1} km", km(m));
    }
    let dem_missing = if dem.samples > 0 {
        dem.missing_samples as f64 / dem.samples as f64 * 100.0
    } else {
        0.0
    };
    println!(
        "  DEM missing samples: {:.2} % of {}",
        dem_missing, dem.samples
    );
    let peaks = g
        .anchors()
        .iter()
        .filter(|a| a.kind == AnchorKind::Peak)
        .count();
    println!(
        "  anchors: {} ({} peaks, {} viewpoints)",
        g.anchors().len(),
        peaks,
        g.anchors().len() - peaks
    );
    Ok(())
}
