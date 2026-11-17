# Decisions

Append-only. One entry per decision: date, decision, why, source. Move the
matching question out of `docs/brief.md` when it lands here.

| # | Date | Decision | Why / source |
|---|------|----------|--------------|
| D1 | 2026-11-16 | Ship as a web app, not a native desktop app | Owner: "trail route webapp" |
| D2 | 2026-11-16 | Metric layers are pluggable; paywalls/tiers only after the product is fleshed out | Owner goals |
| D3 | 2026-11-16 | Two levels of control: quick defaults and a power-user surface | Owner goals |
| D4 | 2026-11-16 | Route planning and editing come before the long tail of layers | Owner's rough approach |
| D5 | 2026-11-16 | Keep the layer system behind paywalls possible, but build no billing in v1 | Owner goals |
| D6 | 2026-11-16 | v1 is Switzerland-first. Global coverage only if layer quality holds; the data layer is built so a region can be added, but no region beyond CH is built for v1 | Owner: would rather be excellent for CH than degraded worldwide |
| D7 | 2026-11-16 | Metrics are split into hard/soft constraints (distance, elevation, surface, trail existence) and terms inside the cost function (views, steepness direction, remoteness, forest, technicality) | Owner, from the combining-metrics discussion |
| D8 | 2026-11-16 | Start-point travel budget (~1 h by train or car) and elevation density are routing inputs, not scores | Owner's workflow: they pick a start first, then a hilly loop |
| D9 | 2026-11-16 | v1 is a personal deployment with no accounts: open the site, plan, done. Premium features are a firm goal later if feasible; otherwise donations. No billing infrastructure in v1 | Owner |
| D10 | 2026-11-16 | New Rust implementation; zill is reference material, not a base | Owner |
| D11 | 2026-11-16 | Heavy computation runs server-side and is cached; the client stays lightweight (SolidJS + MapLibre) and never downloads a graph | Owner |
| D12 | 2026-11-16 | Hand-rolled routing engine in Rust; GraphHopper/Valhalla/ORS/BRouter are references, not dependencies | Owner, after comparison in `docs/routing-engines.md` |
| D13 | 2026-11-16 | Generator-first UI, TrailRouter-like: minimal settings, then a browsable set of routes. No map-first or draw-first entry point in v1 | Owner |
| D14 | 2026-11-16 | Drag-to-edit is in v1, both for adjusting a route and for debugging the cost model live | Owner |
| D15 | 2026-11-16 | Generation = feature-anchor seeds, heading-ring fallback where no features exist, then local refinement of the via points. Refinement is the same code as drag editing | Owner, after options A/B/C |
| D16 | 2026-11-16 | All walkable OSM ways are candidates. Official-network membership is a strong bonus; `informal=yes`, missing `sac_scale`/`trail_visibility` and OSM-only ways are penalised. Each route shows the share on official trails | Owner |
| D17 | 2026-11-16 | `docker compose up` runs the full local stack. The server ships as a lightweight image suitable for Kubernetes. Only a Dockerfile and compose file; no k8s manifests or other deployment config | Owner |
| D18 | 2026-11-16 | Offline import produces one flat mmap'd graph file. A single server binary serves the API and the static client. Routing is plain A*, no CH in v1. Overlay PMTiles come with the layer milestone | Architecture proposal; revisit if generation latency is too high |
| D19 | 2026-11-16 | Travel-time reachability is deferred. Any map point can be a start; the travel-limit control is not in v1. Supersedes the start-budget half of D8 | Owner |
| D20 | 2026-11-16 | Distance is the target, best-effort with closest matches shown. Climb is optional and soft (a small term in the refinement objective), never a filter. Supersedes the elevation-density half of D8 | Owner |
| D21 | 2026-11-16 | Base map is swisstopo WMTS fetched directly by the browser; the server does not proxy tiles. Our own overlay tiles are PMTiles served by `trail-server` when layers land | Verified CORS `*` and CDN caching on `wmts.geo.admin.ch`; OGD terms allow it with attribution |
| D22 | 2026-11-16 | When implementation starts it runs subagent-driven: a fresh implementer per task, a fresh reviewer before the next, whole-branch review at the end. **Paused before execution by the owner; the project is still in design iteration** | Owner; graph/cost interfaces are load-bearing and mistakes compound |
| D23 | 2026-11-16 | No PWA or offline support. The app requires connectivity; no service-worker offline mode and no client-side graph | Owner |
| D24 | 2026-11-18 | Implementation starts with the scaffold only: Rust `trail-server` (axum, `/api/health`, serves `web/dist`) and the Vite + Solid + MapLibre client with map navigation, Topo/Satellite base maps and route-line rendering from synthetic data. No routing algorithm yet. Partially lifts the D22 pause | Owner: "Do not start working on the backend algorithm yet. I want to flesh out the basic design and project structure first" |
| D25 | 2026-11-18 | The interface is designed for the public from the start (no explanation needed), and phone use is first-class mobile web, equal to desktop. Refines D9 (still a personal deployment, no accounts) and Q11 ("no mobile" means no native/PWA app, D23) | Owner, impeccable init interview |
| D26 | 2026-11-18 | "Routes drawn" in the scaffold means rendering route lines (candidates plus a selected one); no manual click-to-draw, consistent with D13 | Owner |
| D27 | 2026-11-18 | Visual direction: "Warm Consumer App, Stone & Course Purple". Warm stone ground, soft white cards, Nunito Sans, purple reserved for the active route and pressable things. Power controls follow "explain, then adjust": the selected route explains its score in bars and each bar opens its own setting inline. Contract in `.impeccable/surfaces/web-src-app-tsx.md` | Owner, impeccable direction rounds (re-rolled once with the steer "I like the Warm Consumer App approach… colors open", then chose card C) |
| D28 | 2026-11-18 | On phones the panel is a draggable bottom sheet with peek / half / full snap points; the map stays full screen | Owner, chose "Draggable bottom sheet" over a Map/Routes switch |
