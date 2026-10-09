# Terrain and sensitivity verification

Windows x64 / NVIDIA RTX 3060. Twenty-one logic tests passed, including negative-coordinate continuity, building foundations, bounded chunk cache eviction, unrestricted exploration and player-relative spawns. Clippy passed with warnings treated as errors. The executable built inside `target/iteration` after Windows denied replacing the older executable; no running user game was terminated.

Rendered checks used the new executable and ordinary movement/transaction paths:

- A 2,400-frame diagonal terrain walk completed 39.910 world seconds and reached (135.5, -135.5), with sampled ground height -0.15. The path crosses the former arena boundary and multiple streamed chunks without teleporting.
- Settings stayed open for 120 rendered frames with world time exactly 0.000; no movement occurred. The visible settings dialog exposes mouse sensitivity, a reset action and close controls. Default sensitivity is 0.50 of the original value.
- The actual shop widget input smoke still bought Light Frame for 20 coins and started wave two.
- The nine-session terrain smoke batch completed all eight waves for every policy/seed combination. Purchases range from 30 to 33 per session. See `terrain-smoke/report.md` and summaries. A replay of mobile seed 11 matches the report. These are smoke tests, not a balance conclusion; terrain exploration and player-relative spawning change encounter pacing.

Generation uses two scales of seeded value noise to set pad heights, then smooth interpolation between flat cores. The ground is a regular triangle surface with shared seam samples, independent of destructible voxel objects. Ground height is reproduced from the run seed; no terrain cells or destruction state exist. The cache generates only nearby chunks and evicts distant ones. Existing ruins are on the original flat starting clearing; procedural placement of new buildings/scenery remains future work. Combat collision and targeting still use horizontal positions.

Mouse sensitivity saves inside this project to `saves/settings.json`, loads safely with defaults for malformed values, and does not alter combat or world clocks. F10/ Escape close settings and return to the underlying screen; mouse capture is disabled while the dialog is open.
