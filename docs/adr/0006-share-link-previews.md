# 0006: Share links carry a preview image of the loop

Date: 2026-11-24. Source: owner.

## Decision

A shared link shows a preview in chat apps: a 1200×630 image of the loop over
relief shading with the 360° view overlay, titled "Loop A · 15.6 km · 743 m
climb".

- Share links move from the URL hash to the query (`/?s=…&d=…`, same
  parameters as D95). Preview crawlers never send the hash, so the server
  couldn't see the loop otherwise. Old `#` links still open, without a preview.
- The server renders the image in Rust, with no browser: it regenerates the
  loop (and re-routes an edited copy through its points), draws hillshade from
  Mapterhorn terrain tiles, the view overlay from `view.bin`, and the loop in
  the active-loop red with a white casing. There are no roads or labels, since
  drawing the outdoor vector map would need a map renderer.
- `GET /` with a share query adds `og:` and `twitter:card` tags. The image is
  `GET /api/og.jpg?<query>`, a JPEG at about 150 kB (a PNG was about 1 MB,
  too big for some chat apps). Previews are cached in memory per link.

## Why

The owner asked for a thumbnail showing the route on relief with views, and
chose query links, server-side rendering, and distance and climb in the title.

## Addendum: `;` is sent as `%3B`

The deployed proxy treats a raw `;` in the query as a separator, so `p=` and
`z=` reached the server cut short, and edited copies got no preview. Links
written before this fix have the same problem; the client still opens them.
