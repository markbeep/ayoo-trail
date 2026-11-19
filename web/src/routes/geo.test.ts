// Run with `node --test web/src/routes/geo.test.ts` (Node strips the types).
import { test } from "node:test";
import assert from "node:assert/strict";
import { legAt, waypointIndices } from "./geo.ts";

// A square loop east, north, west, south: start at [0,0], vias at the far corners.
const coords = [[0, 0], [1, 0], [2, 0], [2, 1], [2, 2], [1, 2], [0, 2], [0, 1], [0, 0]].map(([x, y]) => [x! * 0.01, y! * 0.01]);
const at = (x: number, y: number) => ({ lon: x * 0.01, lat: y * 0.01 });
const wps = [at(0, 0), at(2, 0), at(2, 2), at(0, 0)];

test("waypoints map to increasing coordinate indices, loop ends at the end", () => {
  assert.deepEqual(waypointIndices(coords, wps), [0, 2, 4, 8]);
});

test("a coordinate falls on the leg between the waypoints around it", () => {
  assert.equal(legAt(coords, wps, 1), 0);
  assert.equal(legAt(coords, wps, 3), 1);
  assert.equal(legAt(coords, wps, 6), 2);
  assert.equal(legAt(coords, wps, 8), 2);
});
