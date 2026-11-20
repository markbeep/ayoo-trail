// Run with `node --test web/src/routes/gpx.test.ts` (Node strips the types).
import { test } from "node:test";
import assert from "node:assert/strict";
import { nearestOnRoute, toGpx } from "./gpx.ts";
import type { Route } from "./types.ts";

// A straight track north along 9.1° E, a point every ~111 m.
const coords = Array.from({ length: 11 }, (_, i) => [9.1, 47.3 + i * 0.001, 500 + i]);
const route = { geometry: { type: "LineString", coordinates: coords } } as unknown as Route;

test("nearest track point and its distance", () => {
  const n = nearestOnRoute(coords, { lat: 47.3051, lon: 9.1001 });
  assert.equal(n.i, 5);
  assert.ok(n.d < 20, `${n.d}`);
});

test("checkpoints become wpt on track points, in route order, far ones skipped", () => {
  const gpx = toGpx(route, "Loop A", [
    { lat: 47.308, lon: 9.1, name: "Second" },
    { lat: 47.302, lon: 9.1002, name: "Water & <toilet>" },
    { lat: 47.4, lon: 9.1, name: "Far" },
  ]);
  const wpts = [...gpx.matchAll(/<wpt lat="([\d.]+)" lon="([\d.]+)"><ele>([\d.]+)<\/ele><name>([^<]*)<\/name><\/wpt>/g)];
  assert.deepEqual(wpts.map((m) => [m[1], m[2], m[3], m[4]]), [
    ["47.302000", "9.100000", "502.0", "Water &amp; &lt;toilet&gt;"],
    ["47.308000", "9.100000", "508.0", "Second"],
  ]);
  assert.ok(!gpx.includes("Far"));
  assert.ok(gpx.indexOf("<wpt") < gpx.indexOf("<trk>"), "GPX 1.1 puts wpt before trk");
});

test("unnamed checkpoints are numbered", () => {
  assert.match(toGpx(route, "x", [{ lat: 47.301, lon: 9.1, name: " " }]), /<name>CP1<\/name>/);
});
