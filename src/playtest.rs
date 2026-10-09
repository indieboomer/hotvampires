use crate::{
    content::Config,
    sim::{Game, Phase},
};
use glam::Vec2;
use serde::{Deserialize, Serialize};
use std::{io::Write, path::Path, time::Instant};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunConfig {
    pub seeds: Vec<u64>,
    pub policies: Vec<String>,
    pub max_virtual_seconds: f32,
    pub game: Config,
}
impl Default for RunConfig {
    fn default() -> Self {
        Self {
            seeds: vec![11, 29, 47],
            policies: vec!["mobile".into(), "cautious".into(), "experimental".into()],
            max_virtual_seconds: 900.0,
            game: Config::default(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Report {
    #[serde(default)]
    pub build_id: String,
    #[serde(default)]
    pub data_hash: String,
    pub seed: u64,
    pub policy: String,
    pub policy_version: String,
    pub outcome: String,
    pub virtual_seconds: f32,
    pub world_seconds: f32,
    pub wave: u32,
    pub hp: f32,
    pub currency: u32,
    pub metrics: crate::sim::Metrics,
    pub elapsed_ms: f64,
    #[serde(default)]
    pub terrain_update_ms: f64,
    #[serde(default)]
    pub navigation_update_ms: f64,
    #[serde(default)]
    pub p95_simulation_ms: f64,
    #[serde(default)]
    pub max_simulation_ms: f64,
    #[serde(default)]
    pub terrain_digest: u64,
    #[serde(default)]
    pub terrain_revision: u64,
    pub config: Config,
    pub platform: String,
    pub privileged: bool,
}

pub struct Replay {
    pub game: Game,
    events: Vec<serde_json::Value>,
    cursor: usize,
    input: Vec2,
    pub expected: Report,
}
impl Replay {
    pub fn load(path: &Path) -> Result<Self, String> {
        let expected: Report = serde_json::from_slice(
            &std::fs::read(path.with_extension("json")).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        if !expected.build_id.is_empty() && expected.build_id != env!("HV_SOURCE_HASH") {
            return Err(
                "Replay source fingerprint differs; generate a fresh run with this build".into(),
            );
        }
        let events = std::fs::read_to_string(path)
            .map_err(|e| e.to_string())?
            .lines()
            .map(|s| serde_json::from_str(s).map_err(|e| e.to_string()))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            game: Game::new(expected.seed, expected.config.clone()),
            events,
            cursor: 0,
            input: Vec2::ZERO,
            expected,
        })
    }
    pub fn step(&mut self) -> Result<(), String> {
        while self.cursor < self.events.len()
            && self.events[self.cursor]["real"].as_f64().unwrap_or(0.0)
                <= self.game.real as f64 + 0.0001
        {
            let e = &self.events[self.cursor];
            let d = &e["data"];
            match e["kind"].as_str().unwrap_or("") {
                "input" => {
                    self.input =
                        serde_json::from_value(d["movement"].clone()).map_err(|e| e.to_string())?
                }
                "purchase" => {
                    let id = d["id"].as_str().ok_or("Missing purchase ID")?;
                    let i = self
                        .game
                        .shop
                        .offers
                        .iter()
                        .position(|o| o.id.as_deref() == Some(id))
                        .ok_or("Replay offer mismatch")?;
                    self.game.buy(i)?;
                }
                "reroll" => self.game.reroll()?,
                "heal" => self.game.heal()?,
                "sale" => self.game.sell(d["id"].as_str().ok_or("Missing sale ID")?)?,
                "wave_start" => self.game.start_wave()?,
                _ => {}
            }
            self.cursor += 1;
        }
        self.game.frame(1.0 / 60.0, self.input);
        self.game.events.clear();
        Ok(())
    }
    pub fn finished(&self) -> bool {
        matches!(self.game.phase, Phase::Victory | Phase::Dead)
            || self.game.real >= self.expected.virtual_seconds
    }
}
pub fn replay_headless(path: &Path) -> Result<(), String> {
    let mut replay = Replay::load(path)?;
    for _ in 0..(replay.expected.virtual_seconds * 60.0) as usize + 120 {
        if replay.finished() {
            break;
        }
        replay.step()?;
    }
    if replay.game.metrics.kills != replay.expected.metrics.kills
        || replay.game.metrics.purchases != replay.expected.metrics.purchases
        || (replay.game.hp - replay.expected.hp).abs() > 0.01
        || (replay.expected.terrain_digest != 0
            && (replay.game.terrain.digest() != replay.expected.terrain_digest
                || replay.game.terrain.revision != replay.expected.terrain_revision))
    {
        return Err(format!(
            "Replay diverged: HP {} expected {}, kills {} expected {}",
            replay.game.hp,
            replay.expected.hp,
            replay.game.metrics.kills,
            replay.expected.metrics.kills
        ));
    }
    println!(
        "Replay verified: {:?}, {} kills, {} purchases, HP {:.2}",
        replay.game.phase, replay.game.metrics.kills, replay.game.metrics.purchases, replay.game.hp
    );
    Ok(())
}
pub fn movement(game: &Game, policy: &str) -> Vec2 {
    // Observations are filtered by line of sight; the policy sees no future spawns or seeds.
    let visible: Vec<_> = game
        .enemies
        .iter()
        .filter(|e| {
            e.pos.distance(game.player) < 12.0 && game.obstruction(game.player, e.pos).is_none()
        })
        .collect();
    let nearest = visible.iter().min_by(|a, b| {
        a.pos
            .distance_squared(game.player)
            .total_cmp(&b.pos.distance_squared(game.player))
    });
    if policy == "cautious"
        && nearest.is_none_or(|e| e.pos.distance(game.player) > 4.0)
        && game.real % 4.0 < 1.0
    {
        return Vec2::ZERO;
    }
    let mut best = Vec2::ZERO;
    let mut score = f32::NEG_INFINITY;
    for i in 0..16 {
        let angle = i as f32 * std::f32::consts::TAU / 16.0;
        let d = Vec2::new(angle.cos(), angle.sin());
        let mut future = game.player;
        let mut body = game.player_body.clone();
        // Observe current terrain by trying the shared controller, rather than assuming a flat XZ map.
        for _ in 0..18 {
            crate::controller::advance(
                &game.terrain,
                &game.walls,
                &game.physics,
                &mut future,
                &mut body,
                d * 0.1,
                0.02,
            );
        }
        if future.distance_squared(game.player) < 0.04 {
            continue;
        }
        if game.blocked(future, 0.4) {
            continue;
        }
        let danger: f32 = visible
            .iter()
            .map(|e| 1.0 / (future.distance_squared(e.pos) + 0.25))
            .sum();
        let boundary = (13.0 - future.abs().max_element()).min(3.0);
        let clockwise = Vec2::new(-game.player.y, game.player.x).normalize_or_zero();
        let attraction = if game.player.length() < 3.0 {
            d.dot(Vec2::new(
                (game.real * 0.09).cos(),
                (game.real * 0.09).sin(),
            ))
        } else {
            d.dot(clockwise)
        };
        let loot = game
            .loot
            .iter()
            .filter(|p| {
                p.distance(game.player) < 8.0 && game.obstruction(game.player, **p).is_none()
            })
            .map(|p| future.distance(*p))
            .min_by(f32::total_cmp)
            .unwrap_or(4.0);
        let s = boundary * 0.3 - danger * if policy == "cautious" { 4.0 } else { 2.0 }
            + attraction * if policy == "experimental" { -0.8 } else { 0.8 }
            - loot * 0.08;
        if s > score {
            score = s;
            best = d;
        }
    }
    best
}
pub fn shop_actions(game: &mut Game, policy: &str) {
    let mut actions = 0;
    if game.hp < game.stats().hp * 0.5 {
        let _ = game.heal();
        actions += 1;
    }
    while actions < 8 {
        let candidate = game
            .shop
            .offers
            .iter()
            .enumerate()
            .filter_map(|(i, o)| {
                o.id.as_ref()
                    .and_then(|id| game.cards.iter().find(|c| &c.id == id))
                    .filter(|c| game.can_buy(c).is_ok())
                    .map(|c| {
                        let defense = c.tags.iter().any(|t| t == "defense" || t == "mobility");
                        let score = if policy == "experimental"
                            && ["pulse", "ring", "rupture", "capacitor", "armor_plate"]
                                .contains(&c.id.as_str())
                        {
                            6
                        } else if policy == "cautious" && defense {
                            5
                        } else if c.category == "weapon" || c.category == "skill" {
                            4
                        } else if policy == "experimental" && game.level(&c.id) == 0 {
                            3
                        } else {
                            2
                        };
                        (score, i)
                    })
            })
            .max();
        if let Some((_, i)) = candidate {
            let _ = game.buy(i);
            actions += 1;
        } else {
            break;
        }
    }
    if game.currency > game.reroll_price() + 20 && actions < 8 {
        let _ = game.reroll();
        if let Some(i) = game.shop.offers.iter().enumerate().find_map(|(i, o)| {
            o.id.as_ref()
                .and_then(|id| game.cards.iter().find(|c| &c.id == id))
                .filter(|c| game.can_buy(c).is_ok())
                .map(|_| i)
        }) {
            let _ = game.buy(i);
        }
    }
    let _ = game.start_wave();
}
pub fn run(args: &[String]) -> Result<(), String> {
    let value = |key: &str| {
        args.iter()
            .position(|a| a == key)
            .and_then(|i| args.get(i + 1))
    };
    let cfg: RunConfig = if let Some(path) = value("--config") {
        serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?
    } else {
        RunConfig::default()
    };
    cfg.game.validate()?;
    if cfg.seeds.len() * cfg.policies.len() > 100
        || !cfg.max_virtual_seconds.is_finite()
        || cfg.max_virtual_seconds <= 0.0
        || cfg.max_virtual_seconds > 7200.0
        || cfg
            .policies
            .iter()
            .any(|p| !["mobile", "cautious", "experimental"].contains(&p.as_str()))
    {
        return Err("Invalid playtest budget/policies".into());
    }
    let output = Path::new(value("--output").map_or("reports/latest", String::as_str));
    std::fs::create_dir_all(output).map_err(|e| e.to_string())?;
    // A privileged, isolated clock probe is excluded from progression samples.
    let mut probe = Game::new(1, cfg.game.clone());
    for _ in 0..600 {
        probe.frame(1.0 / 60.0, Vec2::ZERO);
    }
    let clock_ok = probe.world > 0.0 && probe.world < 0.6;
    let audit = serde_json::json!({"scenario":"10-real-seconds-standing","experimental":true,"world_seconds":probe.world,"passed":clock_ok,"finding":if clock_ok{"Nonzero extreme slowdown verified"}else{"Standing does not produce extreme slowdown"},"confidence":"high","proposed_experiment":"Restore minimum time scale to 0.03 and repeat identical clock probe"});
    std::fs::write(
        output.join("clock-audit.json"),
        serde_json::to_vec_pretty(&audit).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    let start_all = Instant::now();
    let mut reports = Vec::new();
    for policy in &cfg.policies {
        for &seed in &cfg.seeds {
            let start = Instant::now();
            let mut game = Game::new(seed, cfg.game.clone());
            let mut input = Vec2::ZERO;
            let mut frame_times = Vec::new();
            let mut frame = 0u32;
            let mut log = std::fs::File::create(output.join(format!("{policy}-{seed}.jsonl")))
                .map_err(|e| e.to_string())?;
            while game.real < cfg.max_virtual_seconds
                && !matches!(game.phase, Phase::Victory | Phase::Dead)
            {
                if start_all.elapsed().as_secs() > 120 {
                    return Err("Execution watchdog: 120 seconds; partial JSONL retained".into());
                }
                if game.phase == Phase::Shop {
                    if game.wave == 1 {
                        game.save(&output.join(format!("{policy}-{seed}-shop.json")))?;
                    }
                    game.event("observation", game.observe());
                    shop_actions(&mut game, policy);
                } else {
                    if frame.is_multiple_of(9) {
                        input = movement(&game, policy);
                        game.event("input", serde_json::json!({"movement":input}));
                    }
                    let frame_start = Instant::now();
                    game.frame(1.0 / 60.0, input);
                    frame_times.push(frame_start.elapsed().as_secs_f64() * 1000.0);
                    crate::terrain_regression::audit(&mut game);
                    if frame.is_multiple_of(300) && crate::terrain_regression::trapped(&game) {
                        game.metrics.trapped_reports += 1;
                        game.event("trapped",serde_json::json!({"player":game.player,"y":game.player_body.y,"revision":game.terrain.revision}));
                    }
                    frame += 1;
                }
                for e in game.events.drain(..) {
                    writeln!(log, "{e}").map_err(|e| e.to_string())?;
                }
            }
            let outcome = match game.phase {
                Phase::Victory => "victory",
                Phase::Dead => "death",
                _ => "timeout",
            }
            .to_string();
            frame_times.sort_by(f64::total_cmp);
            let report = Report {
                terrain_digest: game.terrain.digest(),
                terrain_revision: game.terrain.revision,
                terrain_update_ms: game.metrics.terrain_update_us as f64 / 1000.0,
                navigation_update_ms: game.metrics.navigation_update_us as f64 / 1000.0,
                p95_simulation_ms: frame_times
                    .get(frame_times.len() * 95 / 100)
                    .copied()
                    .unwrap_or(0.0),
                max_simulation_ms: frame_times.last().copied().unwrap_or(0.0),
                build_id: env!("HV_SOURCE_HASH").into(),
                data_hash: {
                    let mut h = 0xcbf29ce484222325u64;
                    for b in include_bytes!("../data/content.json")
                        .iter()
                        .copied()
                        .chain(include_bytes!("../data/terrain.json").iter().copied())
                        .chain(include_bytes!("../data/voxel.json").iter().copied())
                        .chain(serde_json::to_vec(&cfg.game).unwrap())
                    {
                        h = (h ^ b as u64).wrapping_mul(0x100000001b3);
                    }
                    format!("{h:016x}")
                },
                seed,
                policy: policy.clone(),
                policy_version: "directions-v2-terrain/0.15s".into(),
                outcome,
                virtual_seconds: game.real,
                world_seconds: game.world,
                wave: game.wave,
                hp: game.hp,
                currency: game.currency,
                metrics: game.metrics.clone(),
                elapsed_ms: start.elapsed().as_secs_f64() * 1000.0,
                config: cfg.game.clone(),
                platform: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
                privileged: false,
            };
            std::fs::write(
                output.join(format!("{policy}-{seed}.json")),
                serde_json::to_vec_pretty(&report).unwrap(),
            )
            .map_err(|e| e.to_string())?;
            println!(
                "{} seed {}: {} wave {} purchases {} kills {}",
                policy,
                seed,
                report.outcome,
                report.wave,
                report.metrics.purchases,
                report.metrics.kills
            );
            reports.push(report);
        }
    }
    std::fs::write(
        output.join("config.json"),
        serde_json::to_vec_pretty(&cfg).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    std::fs::write(
        output.join("reports.json"),
        serde_json::to_vec_pretty(&reports).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    let commit = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    let mut md = format!(
        "# Actual headless playtest results\n\nSource base commit: `{commit}` (working tree may contain changes). Windows x64. Policy decisions every 0.15 virtual seconds. No visual verification is implied.\n\n| Policy | Seed | Outcome | Wave | HP | Kills | Purchases | Earned | Spent | World s | Virtual s | Debris peak |\n|---|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|\n"
    );
    for r in &reports {
        md += &format!(
            "| {} | {} | {} | {} | {:.1} | {} | {} | {} | {} | {:.1} | {:.1} | {} |\n",
            r.policy,
            r.seed,
            r.outcome,
            r.wave,
            r.hp,
            r.metrics.kills,
            r.metrics.purchases,
            r.metrics.earned,
            r.metrics.spent,
            r.world_seconds,
            r.virtual_seconds,
            r.metrics.debris_peak
        );
    }
    md += "\nSmall samples are smoke tests, not evidence of human enjoyment or build balance. JSONL records commands, damage, deaths, transactions and authoritative destruction. Full telemetry attribution, UI replay and stress profiling remain incomplete.\n";
    md += &format!(
        "\nIsolated standing clock probe: 10 real seconds advanced {:.4} world seconds; {}. See `clock-audit.json` (experimental, excluded from progression results).\n",
        probe.world,
        if clock_ok {
            "PASS"
        } else {
            "FAIL - extreme slowdown absent"
        }
    );
    std::fs::write(output.join("report.md"), md).map_err(|e| e.to_string())?;
    Ok(())
}
pub fn compare(args: &[String]) -> Result<(), String> {
    let read = |key: &str| -> Result<Vec<Report>, String> {
        let i = args
            .iter()
            .position(|a| a == key)
            .ok_or(format!("Missing {key}"))?;
        let p = args.get(i + 1).ok_or("Missing path")?;
        serde_json::from_slice(
            &std::fs::read(Path::new(p).join("reports.json")).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())
    };
    let baseline = read("--baseline")?;
    let candidate = read("--candidate")?;
    for a in baseline {
        let b = candidate
            .iter()
            .find(|b| {
                b.seed == a.seed && b.policy == a.policy && b.policy_version == a.policy_version
            })
            .ok_or("Unpaired comparison")?;
        println!(
            "{} {}: {} -> {}, wave {} -> {}, purchases {} -> {}, received {:.1} -> {:.1}",
            a.policy,
            a.seed,
            a.outcome,
            b.outcome,
            a.wave,
            b.wave,
            a.metrics.purchases,
            b.metrics.purchases,
            a.metrics.received,
            b.metrics.received
        );
    }
    Ok(())
}

pub fn stress() -> Result<(), String> {
    let cfg = Config {
        wave_seconds: 600.0,
        contact_damage: 0.0,
        ..Config::default()
    };
    let mut game = Game::new(99, cfg);
    for i in 0..300 {
        let angle = i as f32 * 0.41;
        let pos = Vec2::new(angle.cos(), angle.sin()) * (3.0 + (i % 10) as f32 * 0.8);
        game.enemies.push(crate::sim::Enemy {
            body: crate::controller::Body::at(game.terrain.height(pos)),
            id: i + 1,
            pos,
            prev: pos,
            hp: 1_000_000.0,
            kind: 0,
            clock: 0.0,
            direction: Vec2::ZERO,
            chipped: 0,
        });
    }
    game.next_id = 301;
    let mut times = Vec::new();
    for i in 0..600 {
        while game.shots.len() < 1000 {
            let a = game.shots.len() as f32 * 0.43;
            let p = Vec2::new(a.cos(), a.sin()) * 6.0;
            game.shots.push(crate::sim::Shot {
                y: 0.85,
                prev_y: 0.85,
                velocity_y: 0.0,
                pos: p,
                prev: p,
                velocity: Vec2::new(-a.sin(), a.cos()) * 18.0,
                life: 20.0,
                damage: 0.001,
                hostile: false,
                pierce: 0,
                hit: Vec::new(),
                primary: false,
            });
        }
        if i % 20 == 0 {
            game.chip_wall(0, Vec2::new(-2.5 + i as f32 / 120.0, 3.2), 0.4);
        }
        let start = Instant::now();
        game.frame(1.0 / 60.0, if i < 300 { Vec2::X } else { Vec2::ZERO });
        times.push(start.elapsed().as_secs_f64() * 1000.0);
        game.events.clear();
    }
    times.sort_by(f64::total_cmp);
    let report = serde_json::json!({"privileged":true,"purpose":"CPU diagnostic, excluded from balance evaluation","build_id":env!("HV_SOURCE_HASH"),"platform":format!("{}-{}",std::env::consts::OS,std::env::consts::ARCH),"frames":600,"enemies":300,"projectiles_before_each_frame":1000,"mean_frame_simulation_ms":times.iter().sum::<f64>()/600.0,"p95_frame_simulation_ms":times[570],"max_frame_simulation_ms":times[599],"cover_removed":game.metrics.cover_removed,"debris_peak":game.metrics.debris_peak,"gpu_measured":false});
    std::fs::create_dir_all("reports/stress").map_err(|e| e.to_string())?;
    std::fs::write(
        "reports/stress/cpu.json",
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    println!("{report}");
    Ok(())
}
