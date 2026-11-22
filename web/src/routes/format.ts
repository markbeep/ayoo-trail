export const km = (m: number) => `${(m / 1000).toFixed(1)} km`;
export const meters = (m: number) => `${Math.round(m).toLocaleString("de-CH")} m`;
/** Scores from the engine are floats; show whole points. */
export const score = (s: number) => Math.round(s);
export const pct = (x: number) => `${Math.round(x * 100)} %`;
export const coord = (p: { lat: number; lon: number }) => `${p.lat.toFixed(4)}, ${p.lon.toFixed(4)}`;
