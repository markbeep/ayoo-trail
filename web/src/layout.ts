import { createSignal } from "solid-js";

const mq = window.matchMedia("(max-width: 760px)");
/** True on phone-sized screens, where the panel becomes a bottom sheet. */
export const [narrow, setNarrow] = createSignal(mq.matches);
mq.addEventListener("change", (e) => setNarrow(e.matches));
