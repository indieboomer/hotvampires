# Actual headless playtest results

Source base commit: `c4f594da62ed956903e30436193893640461975e` (working tree may contain changes). Windows x64. Policy decisions every 0.15 virtual seconds. No visual verification is implied.

| Policy | Seed | Outcome | Wave | HP | Kills | Purchases | Earned | Spent | World s | Virtual s | Debris peak |
|---|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| cautious | 11 | victory | 8 | 33.0 | 282 | 31 | 1410 | 900 | 280.0 | 280.0 | 356 |

Small samples are smoke tests, not evidence of human enjoyment or build balance. JSONL records commands, damage, deaths, transactions and authoritative destruction. Full telemetry attribution, UI replay and stress profiling remain incomplete.

Isolated standing clock probe: 10 real seconds advanced 9.9999 world seconds; FAIL — extreme slowdown absent. See `clock-audit.json` (experimental, excluded from progression results).
