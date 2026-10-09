# Actual headless playtest results

Source base commit: `142a322d96288bc6be6dadc0992702d68e071649` (working tree may contain changes). Windows x64. Policy decisions every 0.15 virtual seconds. No visual verification is implied.

| Policy | Seed | Outcome | Wave | HP | Kills | Purchases | Earned | Spent | World s | Virtual s | Debris peak |
|---|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| mobile | 11 | victory | 8 | 68.5 | 244 | 32 | 1220 | 872 | 280.0 | 280.2 | 332 |
| mobile | 29 | victory | 8 | 79.5 | 303 | 30 | 1515 | 895 | 280.0 | 280.2 | 354 |
| mobile | 47 | victory | 8 | 75.0 | 287 | 31 | 1435 | 920 | 280.0 | 280.2 | 580 |
| cautious | 11 | victory | 8 | 30.0 | 540 | 30 | 2700 | 829 | 452.9 | 579.6 | 508 |
| cautious | 29 | victory | 8 | 52.3 | 298 | 33 | 1490 | 945 | 280.0 | 366.1 | 336 |
| cautious | 47 | victory | 8 | 83.2 | 280 | 32 | 1400 | 899 | 280.0 | 363.0 | 324 |
| experimental | 11 | victory | 8 | 67.8 | 250 | 32 | 1250 | 872 | 280.0 | 280.2 | 290 |
| experimental | 29 | victory | 8 | 60.0 | 303 | 33 | 1515 | 945 | 280.0 | 280.2 | 424 |
| experimental | 47 | victory | 8 | 97.1 | 294 | 31 | 1470 | 859 | 280.0 | 280.2 | 360 |

Small samples are smoke tests, not evidence of human enjoyment or build balance. JSONL records commands, damage, deaths, transactions and authoritative destruction. Full telemetry attribution, UI replay and stress profiling remain incomplete.

Isolated standing clock probe: 10 real seconds advanced 0.3000 world seconds; PASS. See `clock-audit.json` (experimental, excluded from progression results).
