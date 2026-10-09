# Verification of the first vertical slice

Source commit: `c4f594d` (game source fingerprint `725ccd3d4addaae2`). Tested on Windows x64 with Rust 1.98.0 and NVIDIA GeForce RTX 3060. Runtime shaders use direct ash Vulkan; GPU rendering is not substituted with another backend.

- `cargo test --offline`: 17 passed, zero failures.
- `cargo clippy --offline --all-targets -- -D warnings`: passed.
- `cargo build --offline`: passed.
- Final smoke batch: all nine runs finished eight waves and killed the final elite; 30–34 purchases per run. See `verified/report.md` and JSON summaries. These results suggest the prototype is forgiving for the tested policies, not that difficulty is tuned.
- Final recorded mobile seed 11 replay: victory, 280 kills, 32 purchases, HP 69.91, matching its summary.
- Rendered widget input: pointer press/release events passed through egui hit testing; Light Frame charged 20 coins (105 → 85), persisted in the shop checkpoint, and the next-wave button entered wave two. This checks widgets and simulation together. Native desktop mouse automation did not generate the same purchase and is not claimed as verified.
- Captures in `visual/` show actual Vulkan output, original box/cell models, harmless breakup fragments, and a breach preserved in the shop. Capture needed interactive desktop access; ordinary window execution worked inside the sandbox.
- Clock audit: ten stationary virtual-real seconds yielded 0.30000 world seconds with the default, versus 9.99993 in the deliberately faulty test configuration. Both tests used the same code. See the separate experimental comparison.
- Privileged CPU stress: 600 virtual frames, 300 high-HP enemies, 1,000 shots replenished before each frame, destruction and a debris peak of exactly 2,000. Mean simulation time 1.077 ms, p95 3.108 ms, maximum 4.453 ms in the development optimized profile. Counts and timings are diagnostics; injection disables contact damage, so this is excluded from balance data. GPU time and simultaneous stress rendering were not measured. Available ordinary screenshot HUDs showed approximately 61–65 FPS; this is a snapshot, not a measured frame-time distribution or 60-FPS guarantee.

Full telemetry schemas, per-weapon damage attribution, offer-exposure analysis, robust malformed-save checks, native-input integration, live resize/focus testing, hardware stress rendering, audio and a final art/readability pass remain. See STATUS.md. No autonomous work is scheduled beyond this session.
