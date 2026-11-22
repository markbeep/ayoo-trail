// Edited copies (D101): dragging Loop A adds A* next to it. Copies keep their own
// waypoints across setting changes; generated loops are re-picked.

interface Loop { id: string; copyOf?: string }

let n = 0;
export const copyId = (of: string) => `${of}*${++n}`;

/** Fresh generated loops, each followed by its copies; copies whose original is gone go last. */
export function mergeCopies<T extends Loop>(fresh: T[], copies: T[]): T[] {
  const out = fresh.flatMap((f) => [f, ...copies.filter((c) => c.copyOf === f.id)]);
  return [...out, ...copies.filter((c) => !fresh.some((f) => f.id === c.copyOf))];
}

/** "Loop A" from the generated id `loop-<i>` (D79); a copy adds "*", then "*2", "*3"… for more copies of one loop. */
export function loopLabel(all: readonly Loop[], r: Loop): { name: string; star: string } {
  const i = Number((r.copyOf ?? r.id).slice("loop-".length));
  const name = `Loop ${String.fromCharCode(65 + i)}`;
  if (!r.copyOf) return { name, star: "" };
  const k = all.filter((x) => x.copyOf === r.copyOf).findIndex((x) => x.id === r.id);
  return { name, star: k > 0 ? `*${k + 1}` : "*" };
}
