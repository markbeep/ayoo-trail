// Run with `node --test web/src/routes/multipliers.test.ts`.
import { test } from "node:test";
import assert from "node:assert/strict";
import { KEY, multiplierOf, parseMultiplier, scrub, weightOf, word, type Term } from "./multipliers.ts";

const near = (a: number, b: number, rel = 0.01) => assert.ok(Math.abs(a - b) <= Math.abs(b) * rel, `${a} vs ${b}`);

test("multipliers are the key factor to the weight", () => {
  assert.equal(multiplierOf("surface", 1), 2);
  assert.equal(multiplierOf("forest", 0), 1);
  near(weightOf("forest", 0.01), 43.71);
  assert.equal(weightOf("surface", 1e9), weightOf("surface", 1000));
  assert.equal(weightOf("slope", 0.5), 0, "hills and technical are strength only");
  for (const t of Object.keys(KEY) as Term[]) near(multiplierOf(t, weightOf(t, 100)), 100);
});

test("typed multipliers are parsed or refused", () => {
  for (const s of ["", "0", "-3", "abc", "Infinity"]) assert.equal(parseMultiplier(s), null, s);
  assert.equal(parseMultiplier("x0.01"), 0.01);
  assert.equal(parseMultiplier("×2"), 2);
  assert.equal(parseMultiplier(" 1e2 "), 100);
});

test("scrubbing moves log or linear and clamps", () => {
  near(scrub(2, 100, { scale: "log", step: 0, min: 0.001, max: 1000 }), 2 * 1.01 ** 100);
  assert.equal(scrub(2, 1e5, { scale: "log", step: 0, min: 0.001, max: 1000 }), 1000);
  assert.equal(scrub(500, 10, { scale: "linear", step: 10, min: 0, max: 5000 }), 600);
  assert.equal(scrub(500, 10, { scale: "linear", step: 10, min: 0, max: 5000, fast: true }), 1500);
  assert.equal(scrub(500, -100, { scale: "linear", step: 10, min: 0, max: 5000 }), 0);
});

test("words say what a weight does", () => {
  assert.equal(word("forest", 1), "prefer");
  assert.equal(word("surface", 1), "avoid");
  assert.equal(word("surface", -1), "prefer");
  assert.equal(word("view", 0), "off");
});
