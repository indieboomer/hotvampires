# Hot Vampires

Current user-directed changes supersede the bounded arena/flat voxel-floor portions of the original brief: terrain is now an unbounded, seeded noise-generated triangle height field with broad flat building patches, and remains indestructible. Existing ruins occupy a flat starting clearing. Characters and destructible objects retain their placeholder voxel style. Mouse motion automatically orbits outside dialogs; camera elevation is 30 degrees. See RUNNING.md and STATUS.md for implemented behavior.

SPECIFICATION / IMPLEMENTATION BRIEF FOR CODEX
Project: colorful voxel arena survivor with movement-dependent time
Technology: Rust + Vulkan, built from scratch
Version: 1.2 — October 9, 2026; destructible voxel world and automated playtesting

MISSION

Build a working single-player game from scratch in Rust, rendered through Vulkan. Deliver a playable vertical slice, not merely a renderer, UI mockup, or slow-motion demonstration. Combine automatic horde combat, character builds, and a between-wave shop with extreme time slowdown while the player stands still.

Functional references: Vampire Survivors for automatic combat, Brotato for waves and shopping decisions, and Superhot for movement-dependent time. Create original code, visuals, names, items, and content. Do not copy assets, interfaces, or distinctive compositions from those games.

BINDING REQUIREMENTS

1. During active gameplay, standing still produces extreme TIME SLOWDOWN, never a complete time stop.
2. There is no manual aiming or firing. Weapons operate automatically.
3. Rotating the camera/world view does not accelerate time.
4. Use a colorful 3D voxel world and simplified 3D voxel characters. Ground/floor terrain is indestructible. Obstacles, buildings, ruins, and enemies are destructible and break into small voxel fragments. This explicitly supersedes the earlier camera-facing 2D character requirement; see Section 14.
5. Build the game from scratch using Rust + Vulkan.
6. The shop, including skills and items, is a core pillar. Include it in the first complete gameplay loop, with meaningful choices, synergies, and trade-offs.
7. Include an automated playtester and tools allowing Codex to run sessions, diagnose problems, and verify improvements. See Section 13.

8. Destruction must remain readable and performant during horde combat and extreme slow motion. Fine visible voxels do not imply a separate physics body for every cell.

All numerical values, limits, and proposed content below are initial tuning suggestions, not previously agreed user requirements. Preserve the binding requirements. Make routine technical decisions independently and document their rationale.

1. PLAYER EXPERIENCE

Loop: fight a wave → earn currency → visit the shop and modify the build → face a harder wave → final encounter or death → results and restart.

Combat is fast and chaotic while moving. When the player stops, threats approach very slowly, allowing camera rotation and escape-route planning. This is not a completely safe pause. Automatic attacks continue at the same slowed world speed as threats.

Movement accelerates the entire fight, not just enemies. Success comes from positioning and builds rather than aiming. The shop should support planning several waves ahead.

2. CAMERA, SPACE, AND CONTROLS

Proposed view: an elevated third-person camera above the arena, at a fixed angle of approximately 55 degrees, with unrestricted yaw rotation around the player. Use orthographic projection by default. Crowd readability takes priority over effects.

WASD: movement relative to the current camera axes projected onto the ground plane.
Q/E and middle-mouse dragging: camera rotation.
Mouse wheel: bounded zoom.
Escape: pause menu.
Mouse in UI: shop, tooltips, and buttons. The cursor does not control weapons.

Rotate the camera, not simulation coordinates. Camera rotation alone must not change entity positions, target selection, range, RNG, or time scale. When movement and camera rotation occur together, movement direction follows the camera-relative controls.

Use actual 3D voxel character models rather than camera-facing character quads. Start with rigid voxel body parts and simple procedural locomotion. Orient characters toward movement or their current behavior; standing still may retain the last facing direction. Automatic attacks do not depend on player-model facing. Camera rotation changes the view of the models, never their world orientation or simulation state. Ground contact shadows help readability. UI labels may remain billboards.

Arena: one flat, indestructible voxel-styled ground surface with destructible obstacles and compact ruin/building structures. No digging, editable terrain, jumping, traversable upper floors, or open-world streaming. Horizontal gameplay collision is sufficient for the first version, while destruction visuals and fragments move in 3D. Destructible geometry must affect movement and line of sight as specified in Section 14.

3. TIME SYSTEM — THE CENTRAL RULE

Separate at least:
- real_dt: UI, camera, input, and diagnostics;
- world_dt: all combat simulation;
- full pause states: shop, pause menu, focus loss, and end screens.

Full pause outside combat is allowed. The prohibition on zero time applies to standing still during an active wave.

Initial parameters: minimum time scale 0.03, maximum 1.0, smooth transition of approximately 0.08 real seconds. Expose these in configuration.

Derive movement activity from movement input after the deadzone, before multiplying velocity by world_dt. Do not derive it from camera rotation, cursor movement, recoil, enemy pushes, or animation. Do not derive it directly from the character's already slowed measured velocity; that creates feedback.

Suggested formulation:
activity = input vector magnitude clamped to [0,1]
target_scale = lerp(min_scale, 1.0, activity)
scale = smooth approach to target_scale using real_dt
world_dt = real_dt * scale

Normalize diagonal keyboard input. Holding movement against a wall counts as attempted movement and restores normal speed. This is an explicit first-version rule, avoiding complicated blockage detection. Document it in the controls/gameplay instructions.

Player movement also uses world_dt. Tune movement responsiveness through the transition duration. Do not bypass the shared clock by moving the player in real time against slowed enemies.

World time controls:
- movement of all entities and projectiles;
- weapon cooldowns, skills, regeneration, damage over time, and buffs;
- AI, attack telegraphs, spawning, and wave timers;
- gameplay animations, loot collection, hit effects, destruction events, fragment motion, and debris lifetimes.

Real time controls the camera, shop UI, menus, tooltips, and real session-duration counter. Do not introduce hidden real-time damage, income, or regeneration.

Use a fixed simulation step with a world-time accumulator and interpolate rendered positions between states. Extreme slow motion must not cause visible stepping. Do not catch up all elapsed wall-clock time after focus loss. Bound catch-up steps and explicitly log overload. Never perform one enormous update when resuming.

Combat continues very slowly while standing still. Do not add a slowdown duration limit, energy meter, or standing-still penalty at this stage.

4. COMBAT AND AUTOMATIC TARGETING

Weapons select targets using world positions. Default rule: nearest living enemy in range and line of sight, with stable ID-based tie-breaking. Destructible cover blocks attacks until breached; Section 14 defines automatic obstacle clearing when it blocks a target. Rotating the camera without changing the world must produce identical targeting results.

Start with at most three weapon slots. Every weapon operates automatically; no attack or skill-activation key. With no target, a cooldown may become ready, but it must not accumulate multiple stored shots. Normally fire once when a target becomes available. Burst attacks must be explicit weapon features.

Initial weapons, using working names:
- Needle: a fast single projectile; scales with attack speed and piercing.
- Pulse: periodic damage around the character; scales with area and damage.
- Satellite: an automatically orbiting object; contact damage with a per-target cooldown.

Initial enemy types:
- chaser: basic crowd pressure;
- charger: clear telegraph and a committed direction before charging;
- shooter: slow, clearly visible projectiles that can be dodged in slow motion;
- one elite variant in the final encounter.

Hitboxes, contact damage, and brief post-hit invulnerability must be consistent and use world time. Use swept projectile collision where checking only the final position could miss a target.

5. WAVES AND ECONOMY

Proposed run: eight waves, approximately 35–60 world seconds each, escalating difficulty, and a final elite encounter. Real duration is longer and depends on slowdown usage. The HUD shows wave time in world seconds; results show both clocks.

Use one shop currency. No separate XP system or random level-up interruptions in the first version: progression primarily happens in the shop.

Enemies drop currency collected within a pickup radius. At wave end, automatically credit remaining dropped currency exactly once. Remaining enemies disappear without kill rewards; remove projectiles. Combat does not continue behind the shop. Do not fully heal the player for free; healing is an explicit shop service.

Open the shop after every wave except the final wave. The final wave leads to victory and results. Currency and purchases belong to the current run. Persistent metaprogression is out of scope for now.

6. SHOP — A CORE PILLAR, NOT AN ADD-ON

Implement the shop early, alongside the first combat → purchase → combat loop. Never defer it to polish.

Distinguish:
A. Weapons: occupy the three weapon slots and change damage delivery.
B. Skills: passive rules/procs changing build behavior, with at most four slots.
C. Items: stat modifiers, including trade-offs; they do not occupy skill slots.

Skills are automatic, including conditional triggers. Do not add aiming or active ability buttons.

Offer four random cards. Each shows category, price, rarity, tags, effect description, limits, and the current level of an owned version. The first shop must guarantee at least one affordable build-changing option, not just healing.

Required features:
- purchasing, with disabled buttons when funds or slots are insufficient;
- paid rerolls with increasing cost within a visit, reset on the next visit;
- locking individual offers against rerolls and carrying them into the next visit;
- purchased offers leave empty spaces until the next reroll;
- selling weapons, skills, and items for a clearly shown reduced refund;
- upgrading owned weapons/skills by purchasing the next level without consuming another slot;
- a visible button to start the next wave;
- paid healing separate from the four offers.

Suggested prices: items 20–45, skills 45–75, weapons 35–65, rerolls starting at 5 and increasing by 5 per subsequent reroll that visit. Tune income so a typical shop offers a choice between two or three ordinary purchases OR one strong purchase and possibly a reroll. This is an economy-pacing goal, not guaranteed income.

Selling returns approximately 40% of the amount actually spent. Upgraded objects track total purchase costs for consistent refunds. Use integer currency and one explicit rounding rule. Prevent buy/sell arbitrage. Purchases and sales are atomic: rapid repeated clicks must not duplicate effects or currency.

Immediately show stat changes and a short confirmation after buying. Hover previews show before/after values, including effective values after caps. Descriptions such as '+10%' must distinguish relative multipliers from percentage points.

Generate offers using seeded RNG; separate shop, combat, and visual-effect RNG streams. Camera activity and time spent inspecting the shop must not change future offers. Exclude fully upgraded entries. Explain incompatibility when an offer is intended for a different weapon type. Preserve variety rather than generating only items already suited to the current build.

7. INITIAL SHOP CONTENT

Implement at least three weapons, six skills, and twelve items. All must have real gameplay effects, not placeholder UI. Keep tuning values in data files.

Example skills:
1. Piercing: projectiles hit an additional target; further levels increase the limit.
2. Echo: every fifth primary weapon activation repeats at reduced power.
3. Rupture: a kill from a primary hit causes a small explosion.
4. Chain: a hit can jump to a limited number of nearby enemies.
5. Inertia: remaining still for at least 0.75 WORLD seconds charges one bonus for the next attack after moving; no stacking or unlimited charging through tiny movement taps.
6. Momentum: continuous movement for two WORLD seconds gradually increases attack speed; the effect fades after movement stops according to an explicit rule.

Inertia and Momentum support different approaches to time management. No skill changes time scale in the first version.

Define skill/weapon compatibility. Echo counts primary activations; secondary attacks cannot trigger another Echo. Explosions and chains do not trigger additional procs by default. Track damage sources and limit event depth to prevent infinite cascades.

Example items:
- Lens: increased range, reduced maximum HP.
- Capacitor: increased damage, reduced attack speed.
- Light Frame: increased movement speed, reduced armor.
- Armor Plate: increased armor, reduced movement speed.
- Magnet: increased pickup radius.
- Core: increased maximum HP without free full healing.
- Stabilizer: increased attack speed.
- Shard: increased critical chance.
- Amplifier: increased critical multiplier.
- Ring: increased effect area.
- Regenerator: increased regeneration per world second.
- Coupon: reduced reroll cost with a lower bound; no unlimited free rerolls.

Three target builds to test:
- projectiles: Needle + Piercing + Chain;
- close range: Pulse + area + armor;
- mobility: Satellite + Momentum + Light Frame.
They need not be perfectly equal, but each should be capable of finishing a run and meaningfully changing player decisions.

Recompute stats from base values, equipment, and effects instead of reversing previous operations when selling. Establish an order: base → flat bonuses → additive percentages → explicit multipliers → caps. Explain armor and critical-hit formulas. Items must never reduce range, attack speed, or movement speed to zero or below. Reducing maximum HP through selling clamps current HP to the new maximum.

8. RUST + VULKAN

Build a custom application and the thin rendering layer this game needs. Do not use Unity, Unreal, Godot, or Bevy, and do not substitute wgpu for Vulkan. Libraries for windowing, math, serialization, audio, allocation, and Vulkan bindings are allowed. Do not build a general-purpose engine or editor.

Inspect the environment and current official documentation before choosing dependencies. Pin compatible versions and commit Cargo.lock. Do not assume crate versions or specific APIs from memory. Target Windows x64; Linux development is acceptable. State which platform was actually tested.

Renderer requirements:
- explicit Vulkan device, queues, surface, and swapchain;
- correct frame synchronization, per-frame resources, resize/minimize handling, and swapchain recreation;
- voxel-derived meshes for ground, structures, and character parts; bounded instanced fragments;
- depth testing for opaque voxel geometry and separate translucent-effect handling;
- texture atlas, readable HUD, and functioning shop UI;
- reproducible shader compilation to SPIR-V;
- development validation layers without requiring their installation for players;
- isolated, documented unsafe blocks and no destruction of resources still in use by the GPU.

Do not start with PBR, ray tracing, elaborate shadows, material editors, or procedural worlds. Prioritize readable rendering and a complete game.

Separate modules for application/states, input, time, simulation, combat, stats, content, shop, rendering, UI, audio, and diagnostics. Simulation and shop tests must run without a window or GPU. Store weapons, skills, items, waves, and prices in data files with validation of IDs, ranges, and references.

9. SAVING, DIAGNOSTICS, AND PERFORMANCE

Save on entering the shop and after transactions: seed, RNG states, wave number, HP, currency, build, upgrades, offers, locks, reroll cost, and schema version. Write atomically. Loading must not reroll the shop. Mid-wave combat saving is not required in the first version.

Debug overlay: FPS, CPU/GPU timings where available, enemy/projectile counts, time scale, world time, real time, and seed. Enable damage and transaction logs on demand.

Include deterministic headless execution and scripted movement/purchase scenarios. Require determinism within the same build and environment; do not promise identical floating-point results on every platform.

Stress-test target: 300 enemies and 1,000 projectiles/gameplay effects. Use a spatial grid or equivalent partitioning for target queries and collision; avoid all-pairs processing. Measure performance on available hardware and document it. Sixty FPS is a target on the measured configuration, not an unverified promise.

10. IMPLEMENTATION ORDER

Stage 1: Rust/Vulkan skeleton, indestructible arena floor, rotating camera, voxel character, input, and time diagnostics.
Stage 2: movement, nonzero slow motion, one weapon and enemy, one destructible obstacle, voxel enemy breakup, currency, and wave completion.
Stage 3: WORKING shop — item and skill purchases, rerolls, locking, selling, and returning to combat with actual purchase effects.
Stage 4: remaining weapons, enemies, synergies, destructible ruins/buildings, all eight waves, and final encounter.
Stage 5: saving/loading, economy tuning, readability, audio, stress tests, and a runnable package.

Run and verify the result after each stage. Do not stop after the renderer or first shop screen. Maintain PLAN.md and a short STATUS.md describing what actually works and what remains.

11. ACCEPTANCE CRITERIA

- After ten real seconds without input, the world has advanced slightly, but not by zero.
- Camera rotation alone changes neither time scale nor automatic target-selection results.
- A full 360-degree camera orbit shows coherent 3D voxel models without rotating their world orientation or changing hitboxes.
- Movement restores speed responsively and independently of FPS.
- Cooldowns, regeneration, damage over time, projectiles, AI, and waves use world time.
- No manual aiming, firing, or skill activation exists.
- Purchases affect actual combat; selling removes the effect; no negative currency or duplicate transactions.
- Rerolls preserve locks and charge correctly; the next visit resets reroll pricing.
- Full slots, maximum levels, insufficient funds, and an empty offer pool are handled correctly.
- Loading restores the shop, currency, and build without new randomization.
- Procs cannot create infinite loops.
- A full run can be completed or lost, then restarted.
- Resizing and focus loss do not cause time jumps or Vulkan resource errors.
- Verification extends beyond logic tests: inspect rendering, play a scenario, and exercise the live UI wherever the environment permits.

Focus automated tests on time, transactions, stats, RNG, saving, and effect ordering. Evaluate gameplay in practice. If GPU or window access prevents visual verification, state the exact limitation rather than claiming a successful visual test.

12. DELIVERABLES

Deliver source code, assets with provenance/licenses, data files, build/run instructions, controls, a runnable build where the environment permits, actual test results, and known limitations.

Start by inspecting the environment, writing a short plan, and implementing Stage 1. Continue through a complete loop with a full shop. Priorities: consistent time → playable combat → shop decisions → readability → additional content.

13. AUTOMATED PLAYTESTER AND ASSISTED GAME IMPROVEMENT

PURPOSE AND BOUNDARIES

This is mandatory project functionality, not a suggested future extension. Give Codex feedback from the running game instead of relying exclusively on code inspection. Support extended autonomous work within a defined session and budget. Do not require a rigid one-game → report → patch ritual: the system can investigate individual situations, run batches, compare data variants, or reproduce a specific bug.

Separate three roles:
- Runtime playtester: plays combat and the shop, producing actions and data. Start with deterministic policies in Rust; an LLM decision every frame is not required.
- Evaluator: aggregates sessions and identifies evidence-backed issues without editing the game.
- Codex in the development environment: analyzes data, forms hypotheses, changes code/configuration, builds, and compares against a baseline.

Do not assume the game can launch a Codex session itself. Provide stable commands and a file contract for an external agent. The initial implementation is driven by an active Codex session invoking those commands. A persistent supervisor requires an explicitly configured runner, permissions, and limits. Do not promise continued operation after the session closes without that mechanism.

OBSERVATION AND ACTION INTERFACE

Design a shared command layer for humans, bots, and replays. The bot must not teleport, directly apply damage, grant currency, or bypass shop validation. Movement uses the same input/time system. Purchases use the same transaction service as the UI.

Gameplay observations include visible enemies and projectiles, their positions and visible telegraphs, player position, HP, weapon readiness, build, obstacles, currency, wave number, time scale, and current shop offers. Do not expose future spawns, RNG seeds, future offers, or hidden AI parameters. Define visibility and observation range so the agent cannot see through walls. Full debug state may exist for diagnosis and tests, but mark runs using it as privileged and exclude them from balance evaluation.

Actions: movement vector, standing still, camera rotation, purchase by offer ID, reroll, lock/unlock, sell, heal, and start next wave. Do not add aiming or manual skill activation.

Two execution modes:
1. Headless: many fast sessions of the same simulation, without Vulkan or rendering.
2. Rendered: the same bot and commands in the real Vulkan build, screenshots at important moments, and UI/readability checks.

Testing a transaction through an API does not prove a shop button can be clicked. A separate rendered test must exercise actual UI input. Headless execution does not validate voxel rendering, destruction readability, animations, or projectile readability.

BOT TIME

The bot makes decisions in virtual player time, corresponding to application real time, not every CPU frame or only in world time. Accelerating headless execution reduces execution cost without increasing the simulated player's reaction frequency. Example: a virtual frame of 1/60 second and a decision every 0.15 seconds, optionally with reaction delay and observation error. These are policy parameters, not assertions about human ability.

World time still follows bot movement and time scale. The bot cannot take unlimited decisions in zero time. Set an action-count budget for the shop, a virtual-real-time limit per run, and a separate execution watchdog. Report timeouts as timeouts, not victories or deaths. This prevents permanent-standing strategies from blocking an experiment.

PLAYTESTER POLICIES

At least three distinct styles, with versioned parameters:
- cautious: avoids nearby threats, frequently slows time, prioritizes defensive purchases;
- mobile/aggressive: collects currency, maintains movement, prioritizes offensive purchases;
- experimental: pursues different target builds, uses controlled randomness, and explores uncommon purchases.

Additional scripted edge cases: prolonged standing, short alternating movement, movement against a wall, camera rotation without movement, surrounding crowds, selling items that affect HP, full slots, repeated purchase clicks, and proc chains.

The combat bot may predict short-term threats and sample several movement directions. Training an RL network is not required. The shop bot should consider price, health, synergy tags, slots, reroll opportunity cost, and the current build. Do not use one unchanging best-purchase list for every character and situation.

Freeze bot versions for before/after comparisons. Evaluate bot improvements separately from game improvements. For poor results, first distinguish bot failure from a game rule, implementation bug, or parameter problem. Do not simplify the game merely because one policy struggles.

TELEMETRY AND REPORTS

Each session records build/commit ID, data hash, policy version, time/difficulty configuration, game and bot seeds, platform, outcome, termination reason, and interruption information.

JSONL events: input or input changes, hits, deaths, damage sources, skill activations, wave transitions, earnings, transactions, and state snapshots. Bound volume through periodic checkpoints and compression. Preserve bug replays and representative runs. Do not require video for every headless session.

Provide a machine-readable JSON report and a human-readable Markdown report containing:
- results by policy, seed, wave, and build, including sample counts;
- survival and HP-loss curves across waves;
- world time, virtual real time, standing-time share, and average time scale;
- damage dealt/received by source and weapon/skill contribution;
- currency earned, spent, and unspent; reroll and healing counts;
- how often each offer was shown, purchased, locked, and sold;
- bot-stated reasons for rejecting purchases, distinguished from objective facts;
- errors, stuck runs, invalid states, and performance issues;
- references to replays, configurations, and screenshots from rendered runs.

Each finding follows: fact → evidence and run ID → possible cause → confidence → proposed experiment. Separate observations from hypotheses. Do not label an item weak just because it rarely appeared. Normalize purchase analysis by exposure and affordability. Do not infer causation from a purchase/win correlation: successful runs may simply buy that item more often.

CODEX EXPERIMENTS AND IMPROVEMENTS

Codex must be able to:
1. Run a short baseline batch and read the report.
2. Reproduce a specific problem or perform a controlled experiment with an imposed build. Mark such runs as experimental and exclude them from ordinary progression statistics.
3. Form one hypothesis and make a limited change.
4. Run correctness checks and compare versions using identical seed sets and policies.
5. Check the result on a separate validation seed set.
6. Keep or revert the change with reasoning and recorded results.

Do not modify many independent systems at once without a clear need. Data variants can be tested in batches without recompiling each time. Fix and verify code defects separately from balance tuning.

Do not minimize difficulty or maximize wins as the sole objective. Evaluate a profile: stability, time consistency, viability of several builds, quality of shop choices, absence of dominant exploits, and absence of abrupt unjustified difficulty spikes. Target survival ranges belong in a separate design configuration; until agreed, report curves and anomalies rather than inventing an objective ideal.

Automatically change only code/data within this specification. Do not alter binding requirements, evaluation criteria, validation sets, or bot policies to conceal worse results. Do not discard poor runs from aggregates. Present fundamental design changes as proposals instead of implementing them unilaterally.

Work on a separate branch or isolated copy, preserving baseline commits and results. Do not overwrite user changes or destructively reset their work. A retained change must document the problem, hypothesis, diff, trials, outcome, and limitations. Do not publish or deploy the game as part of this loop.

BUDGETS AND STOP CONDITIONS

Experiment configuration must bound run counts, execution time, successive patch attempts, log storage, and model cost where the external runner exposes cost measurement. If cost cannot be measured, report that limitation rather than claiming to enforce it.

Initial proposal: three policies × ten tuning seeds, a separate ten validation seeds per policy, and at most three candidate changes per session. Reduce counts for smoke tests. Small samples detect large problems; they do not establish subtle balance improvements. Show variability and paired outcomes, not just one average.

Stop when the budget is exhausted, improvements cease, infrastructure repeatedly fails, or the next change conflicts with game requirements. Save partial results and resume information. Do not run an unbounded self-improvement loop.

MILESTONE INTEGRATION AND ACCEPTANCE

During Stage 1, design commands, clocks, and serializable observations. In Stage 2, the bot plays a wave independently. In Stage 3, it makes purchases and returns to combat. In Stage 4, it plays complete runs and produces comparable reports. In Stage 5, Codex performs a bounded improvement experiment and explains why the change was kept or rejected.

Provide commands equivalent to:
- playtest run --config ... --output ...
- playtest compare --baseline ... --candidate ...
- playtest replay --run ... --render

Adapt and document the exact syntax. Reports and JSON schemas are deliverables. Generate sample reports from actual runs; do not populate them with invented results.

Acceptance: the agent finishes a run or reports death/timeout, makes real purchases, reproduces a meaningful scenario, and produces a report that identifies a controlled defect in a test configuration; the comparison demonstrates its removal. Never put that controlled defect in the default build. Identical build, seed, configuration, and policy reproduce the result in the same environment. Report visual verification separately from headless verification.

Automation can uncover bugs, ineffective purchases, and suspicious balance. Metrics alone cannot establish that humans enjoy the game. Retain short human playtests for readability, pacing, and satisfaction; their feedback can also feed subsequent Codex experiments.

14. VOXEL ART DIRECTION AND DESTRUCTION

DESIGN INTENT

Create a bright, stylized voxel action-adventure atmosphere: clear silhouettes, saturated accents, warm/cool color separation, compact ruins, and simplified characters. The intended appeal is approachable block-built adventure scenery combined with satisfying fine-grained destruction in slow motion. This is an original setting, not a Minecraft adaptation. Do not reproduce its recognizable characters, enemies, textures, interface, branding, or block palette. Final setting, title, and lore remain open; use a neutral working theme of colorful overgrown ruins and abstract construct enemies for the prototype.

Small fragments peeling off a wall, a column crumbling, and an enemy bursting apart should be a signature visual reward. The player can rotate the camera while time is extremely slow to inspect the destruction. Favor strong readable action over photorealism, tiny visual noise, or gore.

This revision replaces the earlier 2D billboard-character proposal with true 3D voxel characters and enemies. Do not implement both character rendering systems. The player shares the simplified voxel visual style but retains a conventional HP system; volumetric player dismemberment is not required. Enemies must visibly chip and break apart. Ground remains indestructible under all attacks.

VOXEL SCALE AND CAMERA READABILITY

Use significantly finer visible cells than large building-block terrain. Establish scale against a character rather than assuming a universal comparison with another game. Initial candidate: a roughly 1.8-unit-tall character, about 20–30 visible cells tall, with cells around 0.06–0.09 world units. These are art-test values, not performance commitments.

At the default gameplay camera, test approximately 2–4 screen pixels per visible cell edge where feasible at 1080p. Screen projection varies with zoom and orientation; check actual screenshots rather than relying only on world-space measurements. Adjust framing or cell size if individual fragments disappear into subpixel noise. Keep the player large enough to read without making the arena unusably cramped. At distant zoom, simplify or merge decorative debris rather than retaining thousands of flickering single-pixel fragments.

Use limited material palettes per object, broad color grouping, and restrained lighting. Enemy bodies, dangerous projectiles, currency, and harmless debris must have distinct silhouettes/colors. Do not make debris look like collectible currency or hostile projectiles. Muzzle/hit flashes should not obscure impending attacks.

DESTRUCTION CONTRACT

Ground/floor: fully indestructible, including beneath buildings. No terrain excavation, holes, falling through the floor, or mining economy.

Obstacles/buildings/ruins: finite voxel volumes with authored materials and durability. Damage removes local cells or compact groups near the impact, exposes interior faces, and eventually opens routes. Destroying a large object must not merely hide the entire intact mesh after its HP reaches zero. The first vertical slice must demonstrate localized damage before complete collapse.

Enemies: conventional authoritative health and simple stable collision volumes, with localized visual chipping on damage and a full voxel breakup on death. Physical dismemberment and per-voxel vital organs are out of scope. Cap pre-death visual erosion to preserve readable living silhouettes. Visual chips must not silently make a living enemy unhittable. Gameplay death and rewards happen exactly once; debris cannot count as another enemy.

Initial destruction model: use compact cells/chunks and bounded damage kernels. A bullet chips a small region; Pulse damages a bounded region; Satellite chips contacted destructible geometry with a per-object cooldown. Define separate terrain-damage coefficients so enemy damage and demolition speed can be tuned independently. Use simple damageable materials such as brittle masonry and tougher structural blocks without introducing a large material simulation.

Structural collapse: implement a deliberately limited support rule. After a destruction event, evaluate connectivity to authored foundations/anchors for the affected object. Unsupported connected sections detach, then break into a bounded visual fragment set or disappear after an appropriate collapse effect. No realistic structural engineering or exhaustive world-wide support computation. Budget this work and process affected objects only.

World changes persist through shops and subsequent waves in the same run; a new run restores the arena. Save damaged-object state, object IDs, and collision-relevant occupancy along with the shop checkpoint. Cosmetic debris may be omitted from saves. Loading must not rebuild destroyed cover or create new rewards.

AUTOMATIC ATTACKS AND COVER

No manual aiming or demolition button. First prefer a visible enemy in range. If there is no visible target but an enemy is within the weapon's targeting range behind destructible cover, the automatic weapon may attack the first destructible obstruction along the route toward that enemy. Make this fallback deterministic and expose its behavior in debugging. Empty scenery must not cause endless shooting when no combat target exists.

Projectiles hit blocking geometry before enemies behind it; implement ordered collision along the traveled segment. When a breach becomes wide enough, movement and line of sight update consistently. Enemy navigation must use the current traversability map. Avoid permanent sealed spawns and inaccessible living enemies.

Area attacks respect intervening cover by default; do not damage enemies through intact walls while claiming cover blocks attacks. Destruction can open a line for subsequent attacks. Define and test the order of damage, cell removal, collision updates, and later attacks. Enemies may use a bounded automatic demolition behavior if they cannot reach the player because of destructible cover; do not let them idle forever behind a wall.

Use low, readable structures and ruins rather than roofs that hide combat. Any visual occlusion treatment, such as fading foreground walls, must not remove collision or change targeting. Start with a layout that minimizes this need.

No currency farming from scenery in the first version. Enemy kills remain the primary income source. If scenery drops are added later, they must be finite, explicitly budgeted, saved, and tested for duplicate rewards.

PERFORMANCE AND PHYSICS BOUNDARIES

Do not represent every voxel as an entity, draw call, or independent rigid body. Use compact voxel data for destructible volumes and surface meshes that omit internal faces. Mesh merging is allowed as long as the fine voxel scale remains readable where relevant. Rebuild only dirty chunks/objects, with bounded update work and safe GPU resource replacement.

Separate authoritative destruction from cosmetic debris:
- authoritative: removed cells, remaining support, traversability, line of sight, enemy health/death;
- cosmetic: most flying cubes, sparks, dust, tiny rubble, and settling fragments.

Cosmetic debris must not block movement, trigger damage, acquire targets, or affect rewards/RNG. It may use simple gravity, limited floor bounces, and settling without full debris-to-debris collision. Floor contact does not damage the floor. Gameplay uses the remaining solid geometry, not hundreds of moving fragments.

Initial budgets to measure and tune: at most about 2,000 active visible debris instances and a bounded emission count per event, for example 32–96 fragments. A large destruction event may remove many more cells than it emits visible particles. Never require one visible fragment per removed cell. Sample colors/positions from destroyed material so the result still reads as genuine breakup.

Fragment velocities, gravity, bounce, animation, and normal lifetime use world time. Fragments therefore hang in slow motion. Enforce an independent hard count budget through recycling/culling/fading oldest or least visible debris; do not allow slow motion to accumulate unbounded particles. Cosmetic load shedding may use rendering time but must not change gameplay. Keep cosmetic RNG separate from gameplay RNG.

Profile the combined workload: the existing 300-enemy/1,000-projectile gameplay stress scenario plus destruction and bounded debris. Measure CPU simulation, support checks, mesh rebuilding, GPU upload, and rendering separately where possible. Document test hardware and results. Reduce decorative detail before dropping collision updates or changing simulation rules.

SHOP INTEGRATION

Keep the shop central. Destruction should create build choices, not just visual spectacle. Initially attach modest structural-damage differences to existing weapons. Once the basic loop works, add a small number of shop options such as:
- Demolition Head: higher structural damage with reduced attack speed;
- Breach Charge: a bounded additional structural blast when a primary hit destroys cover, with no recursive triggering;
- Salvage Shield: a capped temporary defensive effect on a meaningful cover-destruction event, with a world-time cooldown and no currency generation.

These are optional candidate content, not additions that must delay the first playable shop. If implemented, define event thresholds so removing dozens of cells counts as one bounded event, not dozens of rewards. Test direct purchase benefits and opportunity costs. Avoid a mandatory demolition purchase that makes starting builds unable to traverse the arena.

PLAYTESTER EXTENSIONS

Expose observable remaining cover and traversable paths to the bot, without revealing hidden enemies through geometry. Update observations after authoritative destruction. Add policies/scenarios for using cover, opening a blocked route, area destruction, and fighting in an increasingly open arena.

Add metrics: cover removed, routes opened, time spent attacking obstructions, unreachable targets, stale collision/visibility, destruction-source counts, debris peaks, mesh-rebuild spikes, and progression after cover disappears. Separate balance changes due to destroyed cover from pure cosmetic differences. A more open later wave may become easier or harder; measure rather than assume.

Add replays for simultaneous explosions, support collapse, death during destruction, saving/loading after a breach, camera orbit during a breakup, and maximum slow motion under debris load. Check that headless and rendered modes apply the same authoritative destruction events regardless of cosmetic budgets.

ACCEPTANCE AND SCOPE

- The floor cannot be damaged or excavated by any weapon or explosion.
- One wall or ruin can be locally chipped, breached, and partially collapsed.
- The breach correctly changes collision, line of sight, and navigation.
- A living enemy visibly chips and breaks into voxel fragments on death; rewards occur once.
- Camera rotation alone does not move characters or accelerate the destruction simulation.
- Slow motion visibly slows falling/flying fragments without completely freezing them.
- Harmless fragments remain distinguishable from threats and pickups.
- Debris stays within its budget through sustained slow motion and repeated mass destruction.
- Destroyed geometry persists across waves and save/load, and resets for a new run.
- At the default camera, fine voxel detail and major action remain readable in actual screenshots.
- The complete combat/shop loop is retained; this must not turn into an open-ended voxel-engine project.

Implement one destructible object and one enemy breakup early to validate the visual premise. Only then expand to buildings and ruins. Prioritize combat readability, time behavior, and shop decisions over voxel-system breadth.
