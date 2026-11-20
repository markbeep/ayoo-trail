// Run with `node --test web/src/routes/zones.test.ts` (Node strips the types).
import { test } from "node:test";
import assert from "node:assert/strict";
import { haversine } from "./geo.ts";
import { circle, clampRadius, edgePoint, radiusTo, ZONE_MAX_M, ZONE_MIN_M, zoneMode } from "./zones.ts";

const z = { lat: 47.3, lon: 9.1, radius_m: 300 };

test("circle points sit on the radius and the ring is closed", () => {
  const ring = circle(z);
  assert.deepEqual(ring[0], ring[ring.length - 1]);
  for (const p of ring) assert.ok(Math.abs(haversine([z.lon, z.lat], p) - 300) < 3, `${p}`);
});

test("the edge handle is on the circle and dragging it sets the radius", () => {
  assert.ok(Math.abs(haversine([z.lon, z.lat], edgePoint(z)) - 300) < 3);
  const [lon, lat] = edgePoint({ ...z, radius_m: 1200 });
  assert.ok(Math.abs(radiusTo(z, { lat, lon }) - 1200) <= 3);
});

test("radius is clamped", () => {
  assert.equal(clampRadius(1), ZONE_MIN_M);
  assert.equal(clampRadius(1e9), ZONE_MAX_M);
  assert.equal(radiusTo(z, { lat: z.lat, lon: z.lon }), ZONE_MIN_M);
});

test("zone mode follows kind and hardness; old zones are soft avoids", () => {
  assert.equal(zoneMode(z), "penalize");
  assert.equal(zoneMode({ ...z, hard: true }), "block");
  assert.equal(zoneMode({ ...z, kind: "prefer" }), "prefer");
  assert.equal(zoneMode({ ...z, kind: "prefer", hard: true }), "must");
});
