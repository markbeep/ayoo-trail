// The start field (D57): type to search swisstopo places, or press the crosshair to tap the map.
import { createSignal, For, Show } from "solid-js";
import { Crosshair, LoaderCircle, MapPin } from "@lucide/solid";
import { pickingStart, setPickingStart, setStart, start, startName } from "../state";
import { coord } from "../routes/format";
import { searchPlaces, type Place } from "../search";
import { mapController } from "./MapView";

export const startLabel = () => startName() ?? coord(start());

export function StartSearch() {
  const [text, setText] = createSignal<string | null>(null); // null: not editing, show the start
  const [results, setResults] = createSignal<Place[]>([]);
  const [active, setActive] = createSignal(-1);
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  let timer: ReturnType<typeof setTimeout> | undefined;
  let ctrl: AbortController | undefined;
  let input!: HTMLInputElement;

  const open = () => text() !== null && (results().length > 0 || !!error());
  const close = () => { clearTimeout(timer); ctrl?.abort(); setText(null); setResults([]); setActive(-1); setError(null); setBusy(false); };

  const query = (q: string) => {
    setText(q);
    clearTimeout(timer);
    ctrl?.abort();
    if (q.trim().length < 2) { setResults([]); setError(null); setBusy(false); return; }
    timer = setTimeout(async () => {
      const c = (ctrl = new AbortController());
      setBusy(true);
      try {
        const r = await searchPlaces(q.trim(), c.signal);
        if (c.signal.aborted) return;
        setResults(r);
        setActive(r.length ? 0 : -1);
        setError(r.length ? null : "No place found. Try another name, or tap the map.");
      } catch (e) {
        if (c.signal.aborted) return;
        setResults([]);
        setError("Place search isn't reachable right now. You can still tap the map.");
      }
      setBusy(false);
    }, 200);
  };

  const pick = (p: Place) => {
    setStart(p.pos, p.name);
    mapController?.flyTo(p.pos, 14);
    close();
    input.blur();
  };

  const key = (e: KeyboardEvent) => {
    const n = results().length;
    if (e.key === "ArrowDown" && n) { e.preventDefault(); setActive((i) => (i + 1) % n); }
    else if (e.key === "ArrowUp" && n) { e.preventDefault(); setActive((i) => (i - 1 + n) % n); }
    else if (e.key === "Enter") { const p = results()[active()]; if (p) { e.preventDefault(); pick(p); } }
    else if (e.key === "Escape") { close(); input.blur(); }
  };

  return (
    <div class="start-search">
      <div class={["field start-field", { active: pickingStart() || text() !== null }]}>
        <MapPin size={18} />
        <input
          ref={input}
          class="grow start-input"
          type="search" enterkeyhint="search" autocomplete="off" spellcheck={false}
          role="combobox" aria-label="Start: search a place" aria-autocomplete="list"
          aria-expanded={`${open()}`} aria-controls="start-results"
          aria-activedescendant={active() >= 0 && open() ? `start-opt-${active()}` : undefined}
          placeholder={pickingStart() ? "Tap the map to set your start" : "Search a place"}
          value={text() ?? (pickingStart() ? "" : startLabel())}
          onFocus={(e) => { setPickingStart(false); setText(""); e.currentTarget.value = ""; }}
          onInput={(e) => query(e.currentTarget.value)}
          onKeyDown={key}
          onBlur={() => setTimeout(close, 150)} // after a pointer pick on a result
        />
        <Show when={busy()}><LoaderCircle size={16} class="spin muted-icon" /></Show>
        <button class="field-btn" aria-label={pickingStart() ? "Stop picking on the map" : "Pick the start on the map"}
          aria-pressed={`${pickingStart()}`} title="Tap the map instead"
          onClick={() => { close(); setPickingStart(!pickingStart()); }}>
          <Crosshair size={18} />
        </button>
      </div>
      <Show when={open()}>
        <ul class="start-results" id="start-results" role="listbox" aria-label="Places">
          <Show when={error()}>{(m) => <li class="start-empty" role="presentation">{m()}</li>}</Show>
          <For each={results()}>{(p, i) => (
            <li id={`start-opt-${i()}`} role="option" aria-selected={`${active() === i()}`} class={{ on: active() === i() }}
              onPointerDown={(e) => { e.preventDefault(); pick(p); }} onPointerEnter={() => setActive(i())}>
              <span class="start-name">{p.name}</span>
              <span class="start-kind">{p.area ? `${p.kind} · ${p.area}` : p.kind}</span>
            </li>
          )}</For>
        </ul>
      </Show>
    </div>
  );
}
