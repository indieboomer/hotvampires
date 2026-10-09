# Actual headless playtest results

Source base commit: `f7f4b10fe67f26f3fa575f86456d14018ae7be35` (working tree may contain changes). Windows x64. Policy decisions every 0.15 virtual seconds. No visual verification is implied.

| Policy | Seed | Outcome | Wave | HP | Kills | Purchases | Earned | Spent | World s | Virtual s | Debris peak |
|---|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| mobile | 11 | victory | 8 | 77.6 | 274 | 32 | 1370 | 872 | 280.0 | 280.2 | 372 |
| mobile | 29 | victory | 8 | 69.0 | 317 | 31 | 1585 | 920 | 280.0 | 280.2 | 396 |
| mobile | 47 | victory | 8 | 32.5 | 302 | 32 | 1510 | 899 | 280.0 | 280.2 | 342 |
| cautious | 11 | victory | 8 | 47.4 | 287 | 34 | 1435 | 973 | 280.0 | 349.9 | 488 |
| cautious | 29 | victory | 8 | 79.5 | 312 | 30 | 1560 | 895 | 280.0 | 358.5 | 396 |
| cautious | 47 | victory | 8 | 62.2 | 309 | 33 | 1545 | 934 | 280.0 | 355.2 | 324 |
| experimental | 11 | victory | 8 | 77.6 | 274 | 32 | 1370 | 872 | 280.0 | 280.2 | 372 |
| experimental | 29 | victory | 8 | 69.0 | 317 | 31 | 1585 | 920 | 280.0 | 280.2 | 396 |
| experimental | 47 | victory | 8 | 32.5 | 302 | 32 | 1510 | 899 | 280.0 | 280.2 | 342 |

Small samples are smoke tests, not evidence of human enjoyment or build balance. JSONL records commands, damage, deaths, transactions and authoritative destruction. Full telemetry attribution, UI replay and stress profiling remain incomplete.
