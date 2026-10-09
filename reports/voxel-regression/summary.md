# Voxel terrain regression

Privileged setup, normal controller/combat simulation. Headless only; rendered checks are separate.

- staircase ascent/descent: **PASS** — ascent x 7.73, y 1.50; descended to Vec2(-6.3598194, 0.0); grounding 0, penetration 0 (234.0 ms)
- diagonal slope, stop/start, chunk seams: **PASS** — diagonal Vec2(9.150103, 9.150103), y 1.50, stable pauses; grounding 0, penetration 0 (216.9 ms)
- grenade beneath player, minimum-time fall, escape: **PASS** — floor -2, slow fall -0.019, escaped Vec2(10.999388, 0.0); grounding 0, penetration 0 (96.7 ms)
- ordinary crater enter and leave: **PASS** — crossed bowl y -1.63, exit Vec2(9.958785, 0.0); grounding 0, penetration 0 (109.9 ms)
- enemy group loses support and navigates crater exit: **PASS** — 12/12 enemies walked out; nav rebuilds 27; grounding 0, penetration 0 (155.8 ms)
- overlapping blasts, deep blocked pit, bedrock: **PASS** — overlap floor -7.88; honest trapped pit diagnosed 3 times; grounding 0, penetration 0 (70.9 ms)
- camera rim orbit, partial body occlusion, real-time transitions: **PASS** — occluded fade 1.000, restored 0.000; simulation unchanged; grounding 0, penetration 0 (53.9 ms)
- combat across elevations and terrain cover: **PASS** — height projectile hit 1, structure covered target 2; grounding 0, penetration 0 (68.2 ms)
- automatic grenade, overlapping slow-motion/high-load explosions: **PASS** — automatic crater plus 7 edits; 300-enemy slowed load, debris peak 764; grounding 0, penetration 0 (205.4 ms)
- extensive excavation checkpoint, shop/wave retention and new run: **PASS** — 18 edits, bit-exact save/load through shop to wave 2; grounding 0, penetration 0 (300.7 ms)
