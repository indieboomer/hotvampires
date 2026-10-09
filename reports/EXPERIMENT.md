# Controlled clock-defect experiment

Problem: a configuration can accidentally disable the central standing slowdown while combat progression still appears functional. Hypothesis: an isolated ten-second stationary probe catches this independently of bot win rate.

Only `min_scale` differs: 1.0 in the explicitly named test configuration, 0.03 in the candidate/default. Game seed 11 and cautious policy version remain fixed. The privileged probe is excluded from balance results. The measured defective configuration advances approximately 9.99993 world seconds; the candidate advances 0.30000. The generated `clock-audit.json` files contain the evidence, finding, confidence and proposed experiment. The controlled defect never enters normal configuration.

The paired full run comparison is descriptive only: changing available planning time can change purchases and damage received. Neither a victory nor a lower damage total proves that the game is balanced. Keep the default minimum of 0.03 because it satisfies the specified clock contract. Final smoke seeds 29 and 47 also exercise the corrected default; they are smoke coverage, not a sufficiently powered balance validation set.

Visual evidence is separate: client captures show original placeholder geometry, bounded fragments and a wall breach carried into the shop. `--ui-smoke` feeds synthetic pointer events to actual egui widget hit testing and checks transaction persistence and entry into wave two. Attempts to send native desktop clicks failed to generate a purchase; native mouse integration remains unverified.
