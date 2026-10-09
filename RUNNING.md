# Build and play

Windows x64 requires Rust with the MSVC toolchain and a Vulkan-capable graphics driver. Dependencies are pinned in Cargo.toml and Cargo.lock. This workspace built with Rust 1.98.0. No engine, wgpu, external models, or Vulkan SDK shader compiler is required. The build script validates the original WGSL shader and emits SPIR-V through Naga. Naga is a compiler dependency, not a rendering backend.

```powershell
.\launch.cmd
# Or:
cargo run --offline --target-dir target/iteration
# Optimized executable:
.\tools\build.cmd
.\target\release\hot-vampires.exe
```

Remove `--offline` on a fresh machine if the crates have not been downloaded. No installation or files outside the project are needed at runtime. Run from this folder so checkpoints and reports stay here.

The launcher builds into `target/iteration` so Windows can preserve an older running executable while a new iteration is compiled. The current development executable is `target/iteration/debug/hot-vampires.exe`.

WASD moves relative to the camera. Move the mouse horizontally to orbit around the vertical Y axis, without holding any button; relative input permits continuous rotation without reaching the screen edge. The camera sits 30 degrees above the ground. Q/E also rotates, and the wheel zooms. Camera controls are inactive while a pause, shop or results dialog is shown. The cursor is released and visible in dialogs or when focus is lost, and captured automatically when gameplay resumes. Escape pauses and frees the cursor. Standing still advances combat at 3% speed; camera input does not affect it. The world smoothly desaturates to nearly monochrome as combat slows, returning to full color while moving; HUD and shop remain unaffected. Holding movement against a wall counts as attempted movement and restores full speed. All weapons and skills activate automatically. Gold cubes are currency; bright pink cubes are hostile shots; small muted fragments are harmless. F5 restores the most recent shop checkpoint. R restarts after death or victory.

Survive eight 35-world-second waves. The final elite must also die before victory. Every intervening shop offers four cards, paid healing, escalating rerolls, offer locks, selling, and upgrades. Weapons use three slots, skills four, and items no slots. The starter Needle and Grenade have no sale refund. Keep at least one weapon. Cards currently all have common rarity.

Stats are recomputed from the build. Additive modifiers combine before caps; damage and attack speed modifiers are relative multipliers, critical chance is percentage points. Armor scales damage by `10/(10+armor)` when nonnegative; negative armor increases damage by `1-armor/10`. Refunds use integer floor of 40% of actual spend. Increasing max HP never heals; decreasing it clamps current HP. Echo, Rupture and Chain secondary damage cannot recursively trigger procs.

# Playtesting

F10 opens/closes Settings from gameplay or another screen. The mouse-sensitivity slider takes effect immediately and saves to `saves/settings.json` inside this project. Default sensitivity is 0.50× the original camera speed (0.003 radians per pixel), with a 0.10×–2.00× range and a reset button. Settings pauses combat and frees the cursor; Escape closes it without changing an underlying pause/shop state.

Terrain is finite destructible voxel soil in a 64-metre square arena. Protected bedrock lies at -8 metres; a stone perimeter bounds movement. Seeded noise in `data/terrain.json` supplies initial elevations and a flat starting clearing. `data/voxel.json` configures step height (0.55 m), slope limit (50 degrees), snapping (0.55 m), gravity, body size, grenade radius (3.2 m), terrain strength (0.6), cooldown (3 world seconds), and camera response. Data is embedded at build time, so rebuild after edits.

New runs start with Needle and Grenade. Grenades launch automatically toward nearby enemies, collide with actual solid geometry, and remove soil in a vertically scaled spherical bowl. Ordinary crater rims are terraced by further real excavation. Repeated blasts can reach bedrock or leave impassable pits. Faded terrain still blocks movement, projectiles and automatic targeting. Do not expect jumping, fall damage or automatic teleport recovery.

Player and enemies share stepped, swept grounded movement and world-time falling. The camera's real-time body probes, local dithered cutaway and cyan outline keep the player readable while orbiting. Real drops are followed smoothly. Soil bits, support heights and chunk checksums change synchronously; exposed top/interior surfaces rebuild in edited chunks and neighbors before presentation. Cosmetic fragments remain bounded, harmless and independent of authoritative soil.

Shop checkpoints use schema 2 and preserve excavation, physics settings and vertical body state. Old near-arena shop checkpoints migrate to fresh ground. Saves from the previous infinite terrain outside the new arena are rejected. Structures retain authored foundations; soil removal does not trigger structural settlement.

```powershell
cargo test --offline
cargo clippy --offline --all-targets -- -D warnings
cargo run --offline --target-dir target/iteration -- playtest run --config data/playtest-smoke.json --output reports/latest
cargo run --offline --target-dir target/iteration -- playtest compare --baseline reports/controlled-defect --candidate reports/clock-candidate
cargo run --offline --target-dir target/iteration -- playtest replay --run reports/voxel-smoke/mobile-11.jsonl
cargo run --offline --target-dir target/iteration -- playtest replay --run reports/voxel-smoke/mobile-11.jsonl --render
cargo run --offline --target-dir target/iteration -- playtest stress
cargo run --offline --target-dir target/iteration -- playtest terrain
# Rendered crater traversal and orbit diagnostic:
cargo run --offline --target-dir target/iteration -- --crater-demo --frames 1800
& tools/visual-check.ps1 -Crater
# A rendered bot, bounded by frame count:
cargo run --offline --target-dir target/iteration -- --bot --frames 2400
# Rendered terrain exploration through ordinary movement, with no teleporting:
cargo run --offline --target-dir target/iteration -- --terrain-walk --frames 2400
# Actual widget hit testing with synthetic pointer events, not direct purchase calls:
cargo run --offline --target-dir target/iteration -- --checkpoint reports/verified/mobile-11-shop.json --ui-smoke --frames 120
```

Policies decide every nine virtual frames (0.15 seconds). The run budget is at most 100 sessions, 7,200 virtual seconds each, and a 120-second batch execution watchdog. Shop actions are bounded. Headless sessions use the same clock and transactions as the human application. Bot observations filter enemies/projectiles through cover; world debugging is separate. Three policies use different movement and purchase preferences, but they are simple heuristics, not human models. Reports must not be interpreted as proof that a build is enjoyable or balanced.

Each run produces a JSON summary and JSONL events; the batch adds Markdown results, configuration, and an isolated experimental clock audit. Reports include source-content fingerprint, data fingerprint, policy version, seed, platform, outcome, wave HP snapshots, world/virtual clocks, damage totals, income, purchases, cover removal, and debris peak. Replay requires the paired `.json` summary and `.jsonl` events from the same build. Use matching seeds and frozen policy versions for comparisons. Historical reports in `reports/smoke` precede later correctness changes; `reports/verified` is historical; `reports/voxel-smoke` is the voxel-terrain batch. Replays enforce the source fingerprint and verify the final authoritative terrain digest/revision in addition to combat/shop outcomes.

`data/playtest-controlled-defect.json` is explicitly experimental: its minimum scale of 1 disables slowdown. Never use it for normal play. The default is 0.03. The clock audit identifies the faulty configuration and the corrected candidate demonstrates removal; this is a correctness demonstration, not a balance claim.

The stress command injects 300 high-HP enemies and replenishes 1,000 shots for CPU measurement. It disables contact damage and is marked privileged, excluded from progression statistics. It does not measure GPU stress performance.

For development validation, set `HV_VALIDATION=1` before launching. The renderer enables the Khronos layer if installed; players do not need it. Initialization selects a graphics queue that also presents, and renders one synchronized frame at a time. Swapchain recreation waits for the device; presentation semaphores belong to swapchain images. This favors correctness over maximum throughput. Opaque meshes use depth testing. Geometry is batched; wall surface meshes rebuild only when occupancy changes. Cosmetic fragments are bounded batched cubes with shared world-time motion, not rigid bodies.

`tools/visual-check.ps1` captures only the game client into `reports/visual`, and `-Shop` loads a real first-shop checkpoint. Capturing requires access to the interactive Windows desktop. Its optional `-NativePurchase` probe did not work in this session; use the verified `--ui-smoke` widget test and a human click check. Native OS mouse automation is not claimed as passing.

Dependency API choices were checked against the official [ash documentation](https://docs.rs/ash/0.38.0+1.3.281/ash/) and [winit documentation](https://docs.rs/winit/0.30.13/winit/), and the installed egui renderer source. All visual assets are original procedural geometry; egui supplies its licensed default font atlas. Third-party dependencies retain their own licenses.

`playtest terrain` records ten privileged setup scenarios using the normal controller/combat simulation. These cover stairs, diagonal/seam movement, stop/start slopes, support loss, slow falling, crater entry/escape, enemy support loss, overlapping/deep pits, camera occlusion, height combat, high-load slow explosions and excavation checkpoint retention. Headless reports cannot establish visual quality. Rendered orbit/traversal captures and telemetry live in `reports/visual`; reports/VOXEL-TERRAIN.md states what was actually checked.
