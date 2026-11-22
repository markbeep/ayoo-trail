// Run with `node --test web/src/routes/copies.test.ts` (Node strips the types).
import { test } from "node:test";
import assert from "node:assert/strict";
import { copyId, loopLabel, mergeCopies } from "./copies.ts";

const r = (id: string, copyOf?: string) => ({ id, copyOf });

test("copies follow their original, orphans go last", () => {
  const fresh = [r("loop-0"), r("loop-1")];
  const copies = [r("loop-1*1", "loop-1"), r("loop-5*2", "loop-5"), r("loop-0*3", "loop-0")];
  assert.deepEqual(mergeCopies(fresh, copies).map((x) => x.id), ["loop-0", "loop-0*3", "loop-1", "loop-1*1", "loop-5*2"]);
});

test("labels come from the original's id, copies get a star and a number after the first", () => {
  const rs = [r("loop-0"), r("loop-0*1", "loop-0"), r("loop-0*4", "loop-0"), r("loop-2")];
  assert.deepEqual(rs.map((x) => loopLabel(rs, x)), [
    { name: "Loop A", star: "" }, { name: "Loop A", star: "*" }, { name: "Loop A", star: "*2" }, { name: "Loop C", star: "" },
  ]);
});

test("copy ids are unique", () => {
  assert.notEqual(copyId("loop-0"), copyId("loop-0"));
});
