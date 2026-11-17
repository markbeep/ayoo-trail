import { For } from "solid-js";
import type { Route } from "../routes/types";
import { km, loopName, meters } from "../routes/format";
import { selectedId, setSelectedId } from "../state";

export function RouteTiles(props: { routes: Route[] }) {
  return (
    <div class="tiles" role="group" aria-label="Loops">
      <For each={props.routes}>{(r, i) => (
        <button class="tile" aria-pressed={selectedId() === r.id}
          classList={{ on: selectedId() === r.id }} onClick={() => setSelectedId(r.id)}>
          <span class="tile-head"><b>{loopName(i())}</b><b class="tile-score">{r.score}</b></span>
          <small>{km(r.distance_m)} · {meters(r.climb_m)}</small>
        </button>
      )}</For>
    </div>
  );
}
