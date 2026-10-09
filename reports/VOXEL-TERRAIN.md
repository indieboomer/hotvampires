# Destructible terrain verification

The implementation extends the existing Rust/ash Vulkan prototype. Launch with `launch.cmd`; new runs include an automatically fired Grenade. Configuration lives in `data/voxel.json`. All changes, runtime output, saves and captures stay inside this project.

## Implemented behavior

- Finite 64-metre voxel arena, eight metres of soil below the clearing, protected bottom layer and visible stone perimeter. Initial elevations reuse the existing seeded terrain noise.
- Real grenade excavation, exposed interiors and terraced ordinary crater rims. Soil removal, support and collision update synchronously. Checksum-stamped affected chunks and neighbors rebuild before rendering; navigation's local surface/clearance cache invalidates on edits.
- Shared swept capsule movement, continuous rounded-bottom contact on small stairs, 0.55-metre step/snap limits, 50-degree slope limit, obstacle sliding, slope speed correction and world-time gravity. Substantial support loss causes falling. Body visual correction is bounded to 0.1 metre; placeholder feet follow their supporting surfaces.
- Height-aware targeting, ray collision, projectiles, grenade arcs and cover damage. Camera fading never changes combat visibility or solidity.
- Five camera/body occlusion probes, local dithered cutaway, player outline, hysteresis and real-time elevation following. Hidden enemies do not receive the player treatment.
- Terrain persists through waves, shops and schema-2 shop checkpoints. New runs reset it. Valid near-arena historical saves migrate; old distant saves are rejected.

## Actual tests

Windows x64, NVIDIA GeForce RTX 3060. Source fingerprint: `2fe3988af8d52035`.

- `cargo test --offline`: **28 passed**. Includes deterministic simulation, existing combat/shop tests, collision/mesh agreement after excavation, protected bedrock/edges, sliding against large ledges, continuous physical quarter-metre stair contact, stable capsule corner contact, landing on surviving structure tops and rejecting inconsistent voxel saves.
- `cargo clippy --offline --all-targets -- -D warnings`: passed. Offline development build passed in `target/iteration`.
- `playtest terrain`: **10/10 scenarios passed**, with zero invalid grounding and penetration samples. [Detailed results](voxel-regression/summary.md) and [metrics](voxel-regression/summary.json) cover stairs, diagonal/seam traversal, slope stop/start, under-player support removal, slow falling, crater crossing/escape, group falling, overlapping pits, camera transitions, elevation combat, 300-enemy slowed explosion load and extensive excavation save/load.
- A single ordinary blast produced a two-metre crater. After two stationary real seconds, the falling player had descended only about 0.019 metre; after landing, movement walked out without jumping. Twelve of twelve enemies escaped the ordinary group crater. Deep blocked pits stayed blocked and were diagnosed.
- Nine ordinary combat/shop runs: **nine victories**, all reaching wave 8. [Full batch](voxel-smoke/report.md). All nine runs recorded zero grounding, penetration and unexpected grounded vertical-spike samples. Trapped-region probes recorded transient snapshots; they are conservative navigation diagnostics, not assertions that a run is permanently trapped.
- Full headless `mobile-11` replay matched victory, 260 kills, 30 purchases, HP 78.69, and final authoritative terrain digest/revision. A separate **600-frame rendered replay** ran on Vulkan, reaching 9.925 world seconds. This rendered excerpt is not a full visual replay of the run.
- The rendered crater diagnostic used actual movement/combat while the camera orbited: [initial slowed view](visual/crater.png), [second view](visual/crater-2.png), [third view](visual/crater-3.png), [fourth view](visual/crater-4.png). The inspected images show terraced exposed soil, local wall/rim cutaways and a readable player. Native window-only capture excludes overlapping desktop windows. [Rendered telemetry](visual/render-diagnostics.json) sampled 720 frames, five terrain edits, zero grounding/penetration errors and synchronized collision/render state. Peak observed camera follow change was about 0.140 metre per frame. This is automated visual verification, not a human feel assessment.
- Existing synthetic egui purchase/next-wave test passed using a historical schema-1 checkpoint: Light Frame cost 20 coins and wave 2 started. Its first attempt purchased successfully but missed the next-wave click; a repeat passed. Desktop focus/timing makes this automation intermittent. Native desktop mouse automation is not claimed as verified.

## Performance and limitations

The privileged CPU stress run replenished 1,000 projectiles among 300 enemies for 600 frames. Mean simulation cost was 4.68 ms, p95 11.33 ms, maximum 23.32 ms; decorative fragments stayed capped at 2,000. [Measured report](stress/cpu.json). This is CPU measurement, not GPU timing or a balance sample.

The rendered diagnostic recorded a 10.42 ms maximum terrain mesh-update cost, including the initial full build, and 17.40 ms maximum scene generation. Only affected meshes rebuilt after edits: 96 builds included 64 initial chunks. No GPU timestamps were measured. These numbers describe this machine and diagnostic, not a guaranteed frame budget.

Repeated excavation can produce genuinely impassable pits and stall a run; there is no jumping, fall damage, teleport recovery or invisible escape ramp. The deliberate deep-pit fixture verifies this honestly. Earlier progression failures led to controller-aware bot probing and actual-pose navigation connections near rims; the final nine-run batch completed. Navigation remains a conservative single-surface elevation flow field, not a general multi-level navigation mesh. Local terrain-derived data updates incrementally, but route propagation covers the bounded arena. Enemies report unreachable routes and wait; they do not dig arbitrary recovery tunnels. Trapped-region counters require interpretation around transient or narrow regions.

Soil caps collapse immediately when excavated, so this version has no caves or persistent overhangs. Authored ruins remain anchored and do not settle when their soil is removed. Model boxes, CPU-uploaded triangle meshes, dithered transparency and the rectangular player outline remain prototype art/rendering. Grenade balance, narrow-exit behavior, human traversal feel and broader hardware/GPU profiling remain future work.

The earlier flat-occupancy bot failure is preserved in [the initial report](voxel-initial/report.md) and [seed-29 diagnostic](voxel-initial/mobile-29.json). The current policy uses shared-controller probes and explicitly reports trapped terrain. Historical reports are not interchangeable with current source-fingerprinted replays.
