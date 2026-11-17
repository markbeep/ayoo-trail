// Bottom-sheet drag behaviour for narrow screens. Three snap points; the sheet
// follows the finger and settles on the nearest one (or the next one in the
// direction of a quick flick).

import { createSignal } from "solid-js";

export type Snap = "peek" | "half" | "full";
export const [snap, setSnap] = createSignal<Snap>("half");

const PEEK_PX = 188;
export function snapHeight(s: Snap): number {
  const h = window.innerHeight;
  if (s === "peek") return PEEK_PX;
  if (s === "half") return Math.round(h * 0.52);
  return h - 72;
}

export function attachSheetDrag(handle: HTMLElement, sheet: HTMLElement) {
  let startY = 0, startH = 0, lastY = 0, lastT = 0, v = 0, dragging = false;

  handle.addEventListener("pointerdown", (e) => {
    dragging = true;
    startY = lastY = e.clientY;
    lastT = performance.now();
    startH = sheet.getBoundingClientRect().height;
    v = 0;
    handle.setPointerCapture(e.pointerId);
    sheet.classList.add("dragging");
  });
  handle.addEventListener("pointermove", (e) => {
    if (!dragging) return;
    const now = performance.now();
    v = (lastY - e.clientY) / Math.max(1, now - lastT); // px/ms, positive = up
    lastY = e.clientY; lastT = now;
    const h = Math.min(snapHeight("full"), Math.max(120, startH + (startY - e.clientY)));
    sheet.style.height = `${h}px`;
  });
  const end = (e: PointerEvent) => {
    if (!dragging) return;
    dragging = false;
    sheet.classList.remove("dragging");
    const moved = Math.abs(e.clientY - startY) > 6;
    const order: Snap[] = ["peek", "half", "full"];
    let next: Snap;
    if (!moved) {
      // Tap toggles between peek and half.
      next = snap() === "peek" ? "half" : "peek";
    } else if (Math.abs(v) > 0.5) {
      const i = order.indexOf(snap());
      next = order[Math.max(0, Math.min(2, i + (v > 0 ? 1 : -1)))]!;
    } else {
      const h = sheet.getBoundingClientRect().height;
      next = order.reduce((a, b) => (Math.abs(snapHeight(b) - h) < Math.abs(snapHeight(a) - h) ? b : a));
    }
    sheet.style.height = "";
    setSnap(next);
  };
  handle.addEventListener("pointerup", end);
  handle.addEventListener("pointercancel", end);
}
