// Run with `node --test web/src/url.test.ts` (Node strips the types).
import { test } from "node:test";
import assert from "node:assert/strict";
import { decodeShare, encodeShare, type Share } from "./url.ts";

const share: Share = {
  start: { lat: 47.2985, lon: 9.095 },
  km: 15,
  climb: { preset: "hilly" },
  strictness: 0.8,
  weights: { slope: 1, uphill: 0, downhill: -1, surface: 1, tech: 1, technical: -1, official: 1, forest: 2.5, urban: 1, traffic: 0, view: 2 },
  maxSac: 3,
  zones: [{ lat: 47.31, lon: 9.1, radius_m: 300 }],
  checkpoints: [],
  loop: 2,
  wps: [{ lat: 47.2985, lon: 9.095 }, { lat: 47.31, lon: 9.12 }, { lat: 47.2985, lon: 9.095 }],
  penalties: { repeat: 5, avoid_factor: 5, prefer_factor: 0.5, stem_m: 500, max_overlap: 0.5 },
};

test("a share link round-trips", () => {
  assert.deepEqual(decodeShare(`#${encodeShare(share)}`), share);
  assert.deepEqual(decodeShare(encodeShare({ ...share, climb: "max", wps: null })),
    { ...share, climb: "max", wps: null });
});

test("garbage and out-of-range values are rejected", () => {
  assert.equal(decodeShare(""), null);
  assert.equal(decodeShare("#s=not-a-point"), null);
  assert.ok(encodeShare(share).length < 200, encodeShare(share));
  const bad = (patch: Partial<Share>) => decodeShare(encodeShare({ ...share, ...patch }));
  assert.equal(bad({ km: 500 }), null);
  assert.equal(bad({ maxSac: 9 }), null);
  assert.equal(bad({ weights: { ...share.weights, forest: 101 } }), null);
  assert.equal(bad({ weights: { ...share.weights, slope: -1 } }), null);
  assert.equal(bad({ zones: [{ lat: 47, lon: 9, radius_m: 1e6 }] }), null);
  assert.equal(bad({ zones: Array(33).fill(share.zones[0]) }), null);
});

test("zone kinds, hardness and named checkpoints round-trip", () => {
  const s: Share = {
    ...share,
    zones: [
      { lat: 47.31, lon: 9.1, radius_m: 300 },
      { lat: 47.32, lon: 9.1, radius_m: 300, kind: "avoid", hard: true },
      { lat: 47.33, lon: 9.1, radius_m: 300, kind: "prefer", hard: false },
      { lat: 47.34, lon: 9.1, radius_m: 300, kind: "prefer", hard: true },
    ],
    checkpoints: [{ lat: 47.3, lon: 9.1, name: "Hütte; water, 50% & <toilet>" }, { lat: 47.31, lon: 9.11, name: "" }],
  };
  assert.deepEqual(decodeShare(encodeShare(s)), s);
});

test("old links without zone flags or checkpoints still parse", () => {
  const d = decodeShare("#s=47.2985,9.095&d=15&c=hilly&t=0.8&x=3&w=1,0,-1,1,1,-1,1,2.5,1,0&z=47.31,9.1,300");
  assert.deepEqual(d?.zones, [{ lat: 47.31, lon: 9.1, radius_m: 300 }]);
  assert.deepEqual(d?.checkpoints, []);
  assert.equal(d?.weights.view, 0, "links from before the view term keep it off (D100)");
  assert.equal(decodeShare(encodeShare(share).replace("z=47.31,9.1,300", "z=47.31,9.1,300,q")), null);
});

test("power settings round-trip: signed weights, climb off, penalties", () => {
  const s: Share = {
    ...share,
    climb: "off",
    weights: { ...share.weights, forest: -43.71, traffic: 6.64, slope: 12 },
    penalties: { repeat: 0.01, avoid_factor: 100, prefer_factor: 0.001, stem_m: 0, max_overlap: 1 },
  };
  assert.deepEqual(decodeShare(encodeShare(s)), s);
  assert.ok(!encodeShare(share).includes("n="), "default penalties stay out of the link");
  const bad = (p: Partial<Share["penalties"]>) => decodeShare(encodeShare({ ...share, penalties: { ...share.penalties, ...p } }));
  assert.equal(bad({ repeat: 0.001 }), null);
  assert.equal(bad({ avoid_factor: 0.5 }), null);
  assert.equal(bad({ stem_m: 6000 }), null);
});

test("links without penalties get the defaults", () => {
  const d = decodeShare("#s=47.2985,9.095&d=15&c=hilly&t=0.8&x=3&w=1,0,-1,1,1,-1,1,2.5,1,0");
  assert.deepEqual(d?.penalties, { repeat: 5, avoid_factor: 5, prefer_factor: 0.5, stem_m: 500, max_overlap: 0.5 });
});

test("long must-pass loops share up to the server's 80 km", () => {
  assert.equal(decodeShare(encodeShare({ ...share, km: 75 }))?.km, 75);
  assert.equal(decodeShare(encodeShare({ ...share, km: 81 })), null);
});
