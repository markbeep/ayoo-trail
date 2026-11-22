//! trail_calc: trail-running loop generation for Switzerland.
//!
//! Modules: `graph` (the `graph.bin` format), `geodesy`, `snap` (edge
//! snapping), `cost` (profile and scoring), `route` (forward A* and route
//! assembly), `generate` (loops), `import` (offline OSM importer) and `server` (the web shell).

pub mod cost;
pub mod generate;
pub mod geodesy;
pub mod graph;
pub mod import;
mod region;
pub mod route;
pub mod server;
pub mod snap;
pub mod view;
pub mod webcams;
