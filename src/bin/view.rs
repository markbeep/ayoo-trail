//! `trail-calc-view`: swissALTI3D → 100 m view scores → `view.bin` (D100).

use std::error::Error;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use clap::Parser;
use tracing::info;
use tracing_subscriber::EnvFilter;
use trail_calc::import::alti3d::Alti3dTiles;
use trail_calc::import::ch1903::wgs84_to_lv95;
use trail_calc::import::region;
use trail_calc::view::{sample_dem, view_scores, ViewGrid, CELL_M};

/// Paths default to `$DATA_DIR` (default `./data`).
#[derive(Parser)]
struct Args {
    #[arg(long, env = "REGION", default_value = "demo", value_parser = ["demo", "ch"])]
    region: String,
    /// Default `$DATA_DIR/raw/alti3d`.
    #[arg(long)]
    dem: Option<PathBuf>,
    /// Default `$DATA_DIR/view.bin`.
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
            eprintln!("trail-calc-view: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: Args) -> Result<(), Box<dyn Error>> {
    let data = std::env::var_os("DATA_DIR").map_or_else(|| PathBuf::from("data"), PathBuf::from);
    let out = args.out.unwrap_or_else(|| data.join("view.bin"));
    if out.exists() && !args.force {
        println!(
            "{} exists, skipping (use --force to rebuild)",
            out.display()
        );
        return Ok(());
    }
    let dem = args.dem.unwrap_or_else(|| data.join("raw/alti3d"));
    let b = region(&args.region).ok_or("unknown region")?.bbox;
    // All four corners: the WGS84 bbox is not axis-aligned in LV95. Snapped out to whole km.
    let c = [
        (b.min_lat, b.min_lon),
        (b.min_lat, b.max_lon),
        (b.max_lat, b.min_lon),
        (b.max_lat, b.max_lon),
    ]
    .map(|(lat, lon)| wgs84_to_lv95(lat, lon));
    let km = |v: f64, up: bool| {
        if up {
            (v / 1000.0).ceil() * 1000.0
        } else {
            (v / 1000.0).floor() * 1000.0
        }
    };
    let e0 = km(c.iter().map(|p| p.0).fold(f64::MAX, f64::min), false);
    let e1 = km(c.iter().map(|p| p.0).fold(f64::MIN, f64::max), true);
    let n0 = km(c.iter().map(|p| p.1).fold(f64::MIN, f64::max), true);
    let n1 = km(c.iter().map(|p| p.1).fold(f64::MAX, f64::min), false);
    let (w, h) = (((e1 - e0) / CELL_M) as usize, ((n0 - n1) / CELL_M) as usize);

    let t = Instant::now();
    let tiles = Alti3dTiles::open(&dem)?;
    let z = sample_dem(&tiles, e0, n0, w, h);
    info!(w, h, tiles = tiles.tile_count(), decodes = tiles.decode_count(), elapsed = ?t.elapsed(), "dem sampled");

    let t = Instant::now();
    let v = view_scores(&z, w, h, CELL_M);
    info!(cells = v.iter().filter(|&&x| x > 0).count(), elapsed = ?t.elapsed(), "view scores");

    ViewGrid {
        e0,
        n0,
        cell: CELL_M,
        w,
        h,
        v,
    }
    .write(&out)?;
    println!("wrote {}", out.display());
    Ok(())
}
