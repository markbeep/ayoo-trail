# ayoo

A web app that generates scenic trail-running loops from a distance and
preference layers.

## Travel time

**Travel origin**:
A numbered point someone travels from to reach a start.
_Avoid_: home, commute origin

**Time budget**:
The most one-way, door-to-door minutes a travel origin allows.
_Avoid_: commute time, limit

**Shared reach**:
The places that every travel origin reaches within its own time budget.

**Partial reach**:
The places that some, but not all, travel origins reach.

**Travel time** (layer):
The map layer that shows shared and partial reach.
_Avoid_: commute, isochrone (in UI copy)
