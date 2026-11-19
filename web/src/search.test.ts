// Run with `node --test web/src/search.test.ts`.
import { test } from "node:test";
import assert from "node:assert/strict";
import { toPlace } from "./search.ts";

const r = (label: string, origin = "gazetteer") => ({ attrs: { label, lat: 47.45, lon: 8.92, origin } });

test("the italic part of a label becomes the kind", () => {
  assert.deepEqual(toPlace(r("<i>Populated Place</i> <b>Bichelsee</b> (TG) - Bichelsee-Balterswil")), {
    name: "Bichelsee (TG)", kind: "Populated place", area: "Bichelsee-Balterswil", pos: { lat: 47.45, lon: 8.92 },
  });
});

test("labels without a kind fall back to the origin", () => {
  assert.equal(toPlace(r("<b>Bichelsee-Balterswil (TG)</b>", "gg25")).kind, "Municipality");
  assert.equal(toPlace(r("<b>Bichelsee-Balterswil (TG)</b>", "gg25")).area, null);
  assert.equal(toPlace(r("<b>Somewhere</b>", "unknown")).kind, "Place");
});

test("markup and entities never survive into the name", () => {
  assert.equal(toPlace(r("<b>A &amp; B</b><img src=x onerror=alert(1)>")).name, "A & B");
});
