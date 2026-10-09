# Build and play

Windows x64 requires Rust with the MSVC toolchain and a Vulkan-capable graphics driver. Dependencies are pinned in Cargo.toml and Cargo.lock. This workspace built with Rust 1.98.0. No engine, wgpu, external models, or Vulkan SDK shader compiler is required. The build script validates the original WGSL shader and emits SPIR-V through Naga. Naga is a compiler dependency, not a rendering backend.

```powershell
.\launch.cmd
# Or:
cargo run --offline
# Optimized executable:
.\tools\build.cmd
.\target\release\hot-vampires.exe
```

Remove `--offline` on a fresh machine if the crates have not been downloaded. No installation or files outside the project are needed at runtime. Run from this folder so checkpoints and reports stay here.

WASD moves relative to the camera, Q/E and middle mouse drag rotate, and the wheel zooms. Escape pauses. Standing still advances combat at 3% speed; camera input does not affect it. Holding movement against a wall counts as attempted movement and restores full speed. All weapons and skills activate automatically. Gold cubes are currency; bright pink cubes are hostile shots; small muted fragments are harmless. F5 restores the most recent shop checkpoint. R restarts after death or victory.

Survive eight 35-world-second waves. The final elite must also die before victory. Every intervening shop offers four cards, paid healing, escalating rerolls, offer locks, selling, and upgrades. Weapons use three slots, skills four, and items no slots. The starter Needle has no sale refund. Keep at least one weapon. Cards currently all have common rarity.

Stats are recomputed from the build. Additive modifiers combine before caps; damage and attack speed modifiers are relative multipliers, critical chance is percentage points. Armor scales damage by `10/(10+armor)` when nonnegative; negative armor increases damage by `1-armor/10`. Refunds use integer floor of 40% of actual spend. Increasing max HP never heals; decreasing it clamps current HP. Echo, Rupture and Chain secondary damage cannot recursively trigger procs.

# Playtesting

```powershell
cargo test --offline
cargo clippy --offline --all-targets -- -D warnings
cargo run --offline -- playtest run --config data/playtest-smoke.json --output reports/latest
cargo run --offline -- playtest compare --baseline reports/controlled-defect --candidate reports/clock-candidate
cargo run --offline -- playtest replay --run reports/verified/mobile-11.jsonl
cargo run --offline -- playtest replay --run reports/verified/mobile-11.jsonl --render
cargo run --offline -- playtest stress
# A rendered bot, bounded by frame count:
cargo run --offline -- --bot --frames 2400
# Actual widget hit testing with synthetic pointer events, not direct purchase calls:
cargo run --offline -- --checkpoint reports/verified/mobile-11-shop.json --ui-smoke --frames 120
```

Policies decide every nine virtual frames (0.15 seconds). The run budget is at most 100 sessions, 7,200 virtual seconds each, and a 120-second batch execution watchdog. Shop actions are bounded. Headless sessions use the same clock and transactions as the human application. Bot observations filter enemies/projectiles through cover; world debugging is separate. Three policies use different movement and purchase preferences, but they are simple heuristics, not human models. Reports must not be interpreted as proof that a build is enjoyable or balanced.

Each run produces a JSON summary and JSONL events; the batch adds Markdown results, configuration, and an isolated experimental clock audit. Reports include source-content fingerprint, data fingerprint, policy version, seed, platform, outcome, wave HP snapshots, world/virtual clocks, damage totals, income, purchases, cover removal, and debris peak. Replay requires the paired `.json` summary and `.jsonl` events from the same build. Use matching seeds and frozen policy versions for comparisons. Historical reports in `reports/smoke` precede later correctness changes; `reports/verified` is the final verification batch.

`data/playtest-controlled-defect.json` is explicitly experimental: its minimum scale of 1 disables slowdown. Never use it for normal play. The default is 0.03. The clock audit identifies the faulty configuration and the corrected candidate demonstrates removal; this is a correctness demonstration, not a balance claim.

The stress command injects 300 high-HP enemies and replenishes 1,000 shots for CPU measurement. It disables contact damage and is marked privileged, excluded from progression statistics. It does not measure GPU stress performance.

For development validation, set `HV_VALIDATION=1` before launching. The renderer enables the Khronos layer if installed; players do not need it. Initialization selects a graphics queue that also presents, and renders one synchronized frame at a time. Swapchain recreation waits for the device; presentation semaphores belong to swapchain images. This favors correctness over maximum throughput. Opaque meshes use depth testing. Geometry is batched; wall surface meshes rebuild only when occupancy changes. Cosmetic fragments are bounded batched cubes with shared world-time motion, not rigid bodies.

`tools/visual-check.ps1` captures only the game client into `reports/visual`, and `-Shop` loads a real first-shop checkpoint. Capturing requires access to the interactive Windows desktop. Its optional `-NativePurchase` probe did not work in this session; use the verified `--ui-smoke` widget test and a human click check. Native OS mouse automation is not claimed as passing.

Dependency API choices were checked against the official [ash documentation](https://docs.rs/ash/0.38.0+1.3.281/ash/) and [winit documentation](https://docs.rs/winit/0.30.13/winit/), and the installed egui renderer source. All visual assets are original procedural geometry; egui supplies its licensed default font atlas. Third-party dependencies retain their own licenses.
