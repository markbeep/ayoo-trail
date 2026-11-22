// Run with `node --test web/src/map/info.test.ts` (Node strips the types).
import { test } from "node:test";
import assert from "node:assert/strict";
import { logoMinZoom, lv03ToWgs84, toLogoFeatures } from "./info.ts";

test("LV03 to WGS84 matches swisstopo's reference point within ~1 m", () => {
  // Bern, old observatory: swisstopo's worked example.
  const p = lv03ToWgs84(600000, 200000);
  assert.ok(Math.abs(p.lat - 46.95108) < 1e-4 && Math.abs(p.lon - 7.43864) < 1e-4, JSON.stringify(p));
});

test("a logo shows from the zoom where its coarsest resolution is reached", () => {
  // 10 m/px is about zoom 12.4 at Swiss latitudes on 512 px tiles; finer-only logos come later.
  assert.ok(Math.abs(logoMinZoom({ "1": [], "10": [657], "2.5": [] }) - 12.42) < 0.05);
  assert.ok(logoMinZoom({ "1": [] }) > logoMinZoom({ "1": [], "50": [] }));
  assert.equal(logoMinZoom({}), Infinity);
});

test("SchweizMobil logo features become lon/lat points with the logo and route", () => {
  const fc = toLogoFeatures({ features: [
    { geometry: { coordinates: [600000, 200000, 540] }, properties: { resolutions: { "5": [] }, logo_svg: "https://images.schweizmobil.ch/image-svg/a.svg", route: 2, category: "national" } },
    { geometry: { coordinates: [600000, 200000] }, properties: { resolutions: {}, logo_svg: "x.svg", route: 3 } },
  ] });
  assert.equal(fc.features.length, 1, "logos that never show are dropped");
  const f = fc.features[0]!;
  assert.equal(f.properties.logo, "https://images.schweizmobil.ch/image-svg/a.svg");
  assert.equal(f.properties.route, 2);
  assert.ok(Math.abs(f.geometry.coordinates[0] - 7.43864) < 1e-4);
});
