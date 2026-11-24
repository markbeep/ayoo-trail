// Run with `node --test web/src/routes/travel.test.ts` (Node strips the types).
import { test } from "node:test";
import assert from "node:assert/strict";
import { clampMinutes, loadOrigins, MAX_ORIGINS, MAX_MINUTES, MIN_MINUTES, withOrigin } from "./travel.ts";

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
