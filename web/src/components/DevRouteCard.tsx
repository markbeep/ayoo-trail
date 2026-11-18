import { Show } from "solid-js";
import { devBusy, devError, devNoGraph, devWaypoints, undoWaypoint, clearWaypoints } from "../dev/routeDebug";
import { selected } from "../state";

export function DevRouteCard() {
  return (
    <section class="card" aria-label="Route debug">
      <h2>Route debug</h2>
      <p>
        {devWaypoints().length} waypoints. Click the map to add.
        <Show when={devBusy()}> Routing…</Show>
      </p>
      <Show when={devNoGraph()}><p role="alert">No graph loaded — run `just data` and `just import`</p></Show>
      <Show when={devError()}>{(e) => <p role="alert">{e()}</p>}</Show>
      <div class="chips-row">
        <button class="link-btn" onClick={undoWaypoint} disabled={!devWaypoints().length}>Undo</button>
        <button class="link-btn" onClick={clearWaypoints} disabled={!devWaypoints().length}>Clear</button>
      </div>
      <Show when={(selected() as { stats?: unknown } | null)?.stats}>{(s) => (
        <details><summary>Stats</summary><pre>{JSON.stringify(s(), null, 1)}</pre></details>
      )}</Show>
    </section>
  );
}
