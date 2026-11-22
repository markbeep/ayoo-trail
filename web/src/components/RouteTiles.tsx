import { For, Show } from "solid-js";
import type { Route } from "../routes/types";
import { km, meters, score } from "../routes/format";
import { loopLabel } from "../routes/copies";
import { selectedId, setSelectedId } from "../state";

export function RouteTiles(props: { routes: Route[] }) {
  return (
    <div class="tiles" role="group" aria-label="Loops">
      <div class="tiles-head" aria-hidden="true"><span>Loop</span><span>Distance</span><span>Up</span><span>Score</span></div>
      <For each={props.routes}>{(r) => (
        <button aria-pressed={`${selectedId() === r.id}`}
          class={["tile", { on: selectedId() === r.id }]} onClick={() => setSelectedId(r.id)}>
          <b>{loopLabel(props.routes, r).name}<Show when={loopLabel(props.routes, r).star}><sup>{loopLabel(props.routes, r).star}</sup></Show></b>
          <span>{km(r.distance_m)}</span><span>{meters(r.climb_m)}</span><b class="tile-score">{score(r.score)}</b>
        </button>
      )}</For>
    </div>
  );
}
