# Implementation plan

The destructible terrain extension reuses the Vulkan renderer, terrain noise, automatic combat, bounded debris, fixed-step clock, shop and playtester.

1. Authoritative bit-packed excavation, protection layers and synchronous chunk meshes.
2. Shared rounded swept movement, physical stepping, slope limits and bounded snapping.
3. Support reconciliation in the edit tick and world-time falling.
4. Real-time camera following, body visibility probes and localized cutaways.
5. Elevation-aware navigation, projectile collision and automatic attacks.
6. Persisted voxel state, checkpoint validation, headless and rendered regression checks.

These steps are implemented. STATUS.md and reports/VOXEL-TERRAIN.md describe verification and limitations.

References remain Vampire Survivors for automatic attacks, Superhot for movement-dependent time, Minecraft for readable voxel geometry, and Teardown for localized destruction. Models are original procedural placeholders. No engine, large physics framework, or external models were introduced.
