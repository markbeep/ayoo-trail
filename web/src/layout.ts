import { createSignal } from "solid-js";

const mq = window.matchMedia("(max-width: 760px)");
/** True on phone-sized screens, where the panel becomes a bottom sheet. */
export const [narrow, setNarrow] = createSignal(mq.matches);
mq.addEventListener("change", (e) => setNarrow(e.matches));

// iOS scrolls the whole page up to keep a focused field above the keyboard and doesn't always
// scroll back when the keyboard closes, leaving the sheet stranded. The page itself never
// scrolls, so put it back once no field has focus or the keyboard is gone (D80).
const unscroll = () => { if (window.scrollY !== 0) window.scrollTo(0, 0); };
document.addEventListener("focusout", () => setTimeout(() => {
  if (!document.activeElement?.matches("input, textarea, select")) unscroll();
}, 0));
window.visualViewport?.addEventListener("resize", () => {
  if (window.visualViewport!.height >= window.innerHeight - 1) unscroll();
});

/** Fields that bring up a keyboard. */
export const TYPING = "input:not([type=range]):not([type=checkbox]):not([type=radio]):not([type=button]), textarea";
