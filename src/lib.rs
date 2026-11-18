//! trail-route: trail-running loop generation for Switzerland.
//!
//! The web server shell and the graph file format exist so far; cost and
//! routing modules arrive with the engine milestones (`docs/milestones.md`).

pub mod cost;
pub mod geodesy;
pub mod graph;
pub mod route;
pub mod server;
pub mod snap;
