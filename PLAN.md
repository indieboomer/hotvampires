# Implementation plan

1. Pin Rust dependencies, explicit ash Vulkan renderer, camera and shared clocks.
2. Add automatic combat, a finite destructible wall, bounded debris, and wave income.
3. Implement validated shop transactions and checkpoint saving before expanding content.
4. Add three weapons, six skills, twelve items, eight waves, enemy variants, and deterministic bot runs.
5. Check logic, real Vulkan rendering, report actual results and remaining gaps.

The references inform automatic combat (Vampire Survivors), shared movement-dependent time (Superhot), readable block-built scenery (Minecraft), and localized breakup (Teardown). All assets are original procedural boxes/cells. No game engine or wgpu is used. Vulkan is accessed directly through ash. Keep the first arena compact; do not build an editor or general engine.
