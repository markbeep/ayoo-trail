//! `trail-calc-transit`: unzipped GTFS → Saturday-morning connections → `transit.bin` (ADR 0002).

use std::error::Error;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use clap::Parser;
use trail_calc::transit::Transit;

/// Paths default to `$DATA_DIR` (default `./data`).
#[derive(Parser)]
struct Args {
    /// Unzipped GTFS feed. Default `$DATA_DIR/raw/gtfs`.
    #[arg(long)]
    gtfs: Option<PathBuf>,
    /// Default `$DATA_DIR/transit.bin`.
    #[arg(long)]
    out: Option<PathBuf>,
    /// Service date YYYYMMDD; default the feed's busiest Saturday.
    #[arg(long)]
    date: Option<u32>,
    /// Rebuild even if `--out` exists.
    #[arg(long)]
    force: bool,
}

fn main() -> ExitCode {
    match run(Args::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("trail-calc-transit: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: Args) -> Result<(), Box<dyn Error>> {
    let data = std::env::var_os("DATA_DIR").map_or_else(|| PathBuf::from("data"), PathBuf::from);
    let out = args.out.unwrap_or_else(|| data.join("transit.bin"));
    if out.exists() && !args.force {
        println!(
            "{} exists, skipping (use --force to rebuild)",
            out.display()
        );
        return Ok(());
    }
    let gtfs = args.gtfs.unwrap_or_else(|| data.join("raw/gtfs"));
    let t = Instant::now();
    let tr = Transit::build(&gtfs, args.date)?;
    tr.write(&out)?;
    println!(
        "wrote {}: {} stations, {} trips, {} connections, {} footpaths, service date {}, {:.1?}",
        out.display(),
        tr.stations.len(),
        tr.trips,
        tr.conns.len(),
        tr.foot.len(),
        tr.date,
        t.elapsed()
    );
    Ok(())
}
