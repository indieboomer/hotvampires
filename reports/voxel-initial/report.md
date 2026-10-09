# Actual headless playtest results

Source base commit: `f3c582c5123287cbcd9278aa5d2470b19801b893` (working tree may contain changes). Windows x64. Policy decisions every 0.15 virtual seconds. No visual verification is implied.

| Policy | Seed | Outcome | Wave | HP | Kills | Purchases | Earned | Spent | World s | Virtual s | Debris peak |
|---|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| mobile | 11 | victory | 8 | 80.0 | 266 | 29 | 1330 | 875 | 280.0 | 280.2 | 508 |
| mobile | 29 | timeout | 8 | 65.0 | 68 | 13 | 340 | 323 | 901.2 | 900.0 | 292 |
| mobile | 47 | victory | 8 | 50.5 | 269 | 32 | 1345 | 894 | 280.0 | 280.2 | 404 |
| cautious | 11 | victory | 8 | 50.0 | 209 | 28 | 1045 | 810 | 280.0 | 349.6 | 548 |
| cautious | 29 | victory | 8 | 87.2 | 214 | 30 | 1070 | 821 | 280.0 | 352.0 | 427 |
| cautious | 47 | victory | 8 | 75.0 | 235 | 27 | 1175 | 791 | 280.0 | 362.3 | 622 |
| experimental | 11 | victory | 8 | 69.3 | 256 | 29 | 1280 | 885 | 280.0 | 280.2 | 504 |
| experimental | 29 | victory | 8 | 54.5 | 250 | 32 | 1250 | 861 | 280.0 | 280.2 | 500 |
| experimental | 47 | victory | 8 | 79.7 | 227 | 31 | 1135 | 869 | 280.0 | 280.2 | 436 |

Small samples are smoke tests, not evidence of human enjoyment or build balance. JSONL records commands, damage, deaths, transactions and authoritative destruction. Full telemetry attribution, UI replay and stress profiling remain incomplete.

Isolated standing clock probe: 10 real seconds advanced 0.3000 world seconds; PASS. See `clock-audit.json` (experimental, excluded from progression results).
