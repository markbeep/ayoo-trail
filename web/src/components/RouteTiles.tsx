import { For, Show } from "solid-js";
import type { Route } from "../routes/types";
import { km, meters, score } from "../routes/format";
import { loopLabel } from "../routes/copies";
import { selectedId, setSelectedId } from "../state";

export function RouteTiles(props: { routes: Route[] }) {
  return (
    <div class="tiles" role="group" aria-label="Loops">
      <For each={props.routes}>{(r) => (
        <button aria-pressed={`${selectedId() === r.id}`}
          class={["tile", { on: selectedId() === r.id }]} onClick={() => setSelectedId(r.id)}>
          <span class="tile-head"><b>{loopLabel(props.routes, r).name}<Show when={loopLabel(props.routes, r).star}><sup>{loopLabel(props.routes, r).star}</sup></Show></b><b class="tile-score">{score(r.score)}</b></span>
          <small>{km(r.distance_m)} · {meters(r.climb_m)}</small>
        </button>
      )}</For>
    </div>
  );
}
