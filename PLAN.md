# Implementation plan

1. Pin Rust dependencies, explicit ash Vulkan renderer, camera and shared clocks.
2. Add automatic combat, a finite destructible wall, bounded debris, and wave income.
3. Implement validated shop transactions and checkpoint saving before expanding content.
4. Add three weapons, six skills, twelve items, eight waves, enemy variants, and deterministic bot runs.
5. Check logic, real Vulkan rendering, report actual results and remaining gaps.

Current terrain revision: replace the arena floor with a seeded, continuous triangle height field, stream a bounded chunk cache, preserve flat foundations, ground actors/debris, and spawn combat around the exploring player. Verify negative-coordinate seams, cache eviction, unlimited movement, a rendered walk, and the combat/shop loop.

The references inform automatic combat (Vampire Survivors), shared movement-dependent time (Superhot), readable block-built scenery (Minecraft), and localized breakup (Teardown). Characters/objects are original procedural boxes/cells; terrain uses an original noise-generated surface. No game engine or wgpu is used. Vulkan is accessed directly through ash. Generate only the visible surroundings; do not build an editor or general engine.
