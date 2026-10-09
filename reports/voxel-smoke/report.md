# Actual headless playtest results

Source base commit: `f3c582c5123287cbcd9278aa5d2470b19801b893` (working tree may contain changes). Windows x64. Policy decisions every 0.15 virtual seconds. No visual verification is implied.

| Policy | Seed | Outcome | Wave | HP | Kills | Purchases | Earned | Spent | World s | Virtual s | Debris peak |
|---|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| mobile | 11 | victory | 8 | 78.7 | 260 | 30 | 1300 | 895 | 280.0 | 280.2 | 494 |
| mobile | 29 | victory | 8 | 59.7 | 227 | 31 | 1135 | 877 | 292.9 | 293.1 | 360 |
| mobile | 47 | victory | 8 | 85.0 | 285 | 32 | 1425 | 913 | 280.0 | 280.2 | 420 |
| cautious | 11 | victory | 8 | 65.0 | 238 | 28 | 1190 | 855 | 280.0 | 354.1 | 504 |
| cautious | 29 | victory | 8 | 65.7 | 236 | 32 | 1180 | 842 | 280.0 | 352.2 | 647 |
| cautious | 47 | victory | 8 | 85.0 | 257 | 31 | 1285 | 898 | 280.0 | 362.5 | 386 |
| experimental | 11 | victory | 8 | 62.7 | 238 | 29 | 1190 | 850 | 280.0 | 280.2 | 484 |
| experimental | 29 | victory | 8 | 66.2 | 213 | 31 | 1065 | 877 | 280.0 | 280.2 | 517 |
| experimental | 47 | victory | 8 | 59.4 | 273 | 29 | 1365 | 860 | 280.0 | 280.2 | 532 |

Small samples are smoke tests, not evidence of human enjoyment or build balance. JSONL records commands, damage, deaths, transactions and authoritative destruction. Full telemetry attribution, UI replay and stress profiling remain incomplete.

Isolated standing clock probe: 10 real seconds advanced 0.3000 world seconds; PASS. See `clock-audit.json` (experimental, excluded from progression results).
