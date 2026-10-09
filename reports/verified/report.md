# Actual headless playtest results

Source base commit: `c4f594da62ed956903e30436193893640461975e` (working tree may contain changes). Windows x64. Policy decisions every 0.15 virtual seconds. No visual verification is implied.

| Policy | Seed | Outcome | Wave | HP | Kills | Purchases | Earned | Spent | World s | Virtual s | Debris peak |
|---|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| mobile | 11 | victory | 8 | 69.9 | 280 | 32 | 1400 | 872 | 280.0 | 280.2 | 356 |
| mobile | 29 | victory | 8 | 69.0 | 320 | 31 | 1600 | 920 | 280.0 | 280.2 | 388 |
| mobile | 47 | victory | 8 | 85.0 | 314 | 32 | 1570 | 899 | 280.0 | 280.2 | 348 |
| cautious | 11 | victory | 8 | 55.0 | 288 | 34 | 1440 | 973 | 280.0 | 351.7 | 360 |
| cautious | 29 | victory | 8 | 71.8 | 311 | 30 | 1555 | 895 | 280.0 | 358.4 | 320 |
| cautious | 47 | victory | 8 | 53.1 | 313 | 33 | 1565 | 934 | 280.0 | 354.9 | 412 |
| experimental | 11 | victory | 8 | 57.3 | 315 | 32 | 1575 | 883 | 280.0 | 280.2 | 452 |
| experimental | 29 | victory | 8 | 75.8 | 328 | 31 | 1640 | 920 | 280.0 | 280.2 | 360 |
| experimental | 47 | victory | 8 | 98.4 | 309 | 33 | 1545 | 949 | 280.0 | 280.2 | 324 |

Small samples are smoke tests, not evidence of human enjoyment or build balance. JSONL records commands, damage, deaths, transactions and authoritative destruction. Full telemetry attribution, UI replay and stress profiling remain incomplete.

Isolated standing clock probe: 10 real seconds advanced 0.3000 world seconds; PASS. See `clock-audit.json` (experimental, excluded from progression results).
