// Run with `node --test web/src/routes/travel.test.ts` (Node strips the types).
import { test } from "node:test";
import assert from "node:assert/strict";
import { alphaAt, clampMinutes, loadOrigins, MAX_ORIGINS, MAX_MINUTES, MIN_MINUTES, PARTIAL, reachImage, SHARED, timeTo, withOrigin, type Stop } from "./travel.ts";

test("a new travel origin gets 60 min, up to the limit", () => {
  let list = withOrigin([], { lat: 47, lon: 8 });
  assert.deepEqual(list, [{ lat: 47, lon: 8, minutes: 60 }]);
  for (let i = 0; i < 20; i++) list = withOrigin(list, { lat: 47, lon: 8 + i / 100 });
  assert.equal(list.length, MAX_ORIGINS);
});

test("time budgets clamp to whole minutes in range", () => {
  assert.equal(clampMinutes(5), MIN_MINUTES);
  assert.equal(clampMinutes(999), MAX_MINUTES);
  assert.equal(clampMinutes(72.6), 73);
  assert.equal(clampMinutes(Number.NaN), 60);
});

test("stored origins are checked, clamped and cut to the limit", () => {
  assert.deepEqual(loadOrigins(undefined), []);
  assert.deepEqual(loadOrigins("nope"), []);
  const many = Array.from({ length: 12 }, () => ({ lat: 47, lon: 8, minutes: 500 }));
  const got = loadOrigins([{ lat: "x" }, ...many]);
  assert.equal(got.length, MAX_ORIGINS);
  assert.equal(got[0]!.minutes, MAX_MINUTES);
});

// Two origins 0.3° apart (~23 km): each reaches its own spot, a stop between them is reached by both.
const A: Stop[] = [[8.0, 47.0, 0], [8.15, 47.0, 900]];
const B: Stop[] = [[8.3, 47.0, 0], [8.15, 47.0, 600]];

test("time to a spot is the best stop plus the walk, within the budget and last walk", () => {
  assert.equal(timeTo(A, 30, { lon: 8.0, lat: 47.0 }), 0);
  const d = timeTo(A, 30, { lon: 8.15, lat: 47.009 })!; // ~1 km north of the stop: 900 s + ~800 s walk
  assert.ok(d > 1650 && d < 1750, `${d}`);
  assert.equal(timeTo(A, 20, { lon: 8.15, lat: 47.009 }), null); // over budget
  assert.equal(timeTo(A, 240, { lon: 8.15, lat: 47.03 }), null); // > 20 min walk from every stop
});

test("the area is strong where every origin gets, faint where only some do", () => {
  const img = reachImage([A, B], [30, 30]);
  assert.equal(img.data.length, img.width * img.height * 4);
  const [w, s, e, n] = img.bounds;
  assert.ok(w < 8.0 && e > 8.3 && s < 47.0 && n > 47.0);
  assert.equal(alphaAt(img, { lon: 8.15, lat: 47.0 }), SHARED[3]);
  assert.equal(alphaAt(img, { lon: 8.0, lat: 47.0 }), PARTIAL[3]);
  assert.equal(alphaAt(img, { lon: 8.07, lat: 47.0 }), 0);
});
