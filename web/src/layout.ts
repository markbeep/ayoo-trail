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

// Phone keyboards shrink only the visual viewport, so a sheet pinned to the layout viewport's
// bottom ends up under the keyboard (D105). Publish the hidden part as --kb (lifts the sheet)
// and the visible height as --vvh (caps it).
const vv = window.visualViewport;
const syncKeyboard = () => {
  if (!vv) return;
  const root = document.documentElement.style;
  root.setProperty("--kb", `${Math.max(0, Math.round(window.innerHeight - vv.height - vv.offsetTop))}px`);
  root.setProperty("--vvh", `${Math.round(vv.height)}px`);
};
vv?.addEventListener("resize", syncKeyboard);
vv?.addEventListener("scroll", syncKeyboard);
syncKeyboard();

/** Fields that bring up a keyboard. */
export const TYPING = "input:not([type=range]):not([type=checkbox]):not([type=radio]):not([type=button]), textarea";
