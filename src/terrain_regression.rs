//! Privileged terrain fixtures exercise the normal fixed-step controller and combat.
use crate::{
    content::Config,
    controller::{Body, clear, point, support},
    sim::{Enemy, Game, Grenade, Phase},
    terrain::COLS,
};
use glam::{Vec2, Vec3};
use serde::Serialize;
#[derive(Serialize)]
pub struct ResultRow {
    pub scenario: String,
    pub passed: bool,
    pub detail: String,
    pub metrics: crate::sim::Metrics,
    pub terrain_update_us: u64,
    pub navigation_update_us: u64,
    pub elapsed_ms: f64,
}
fn fixture() -> Game {
    let mut g = Game::new(
        42,
        Config {
            wave_seconds: 600.0,
            contact_damage: 0.0,
            ..Config::default()
        },
    );
    g.walls.clear();
    g.build.clear();
    g.spawn = 1e6;
    g
}
fn place(g: &mut Game, p: Vec2) {
    g.player = p;
    g.prev_player = p;
    g.player_body = Body::at(support(&g.terrain, p, g.physics.radius));
}
fn enemy(g: &mut Game, p: Vec2) {
    let id = g.next_id;
    g.next_id += 1;
    g.enemies.push(Enemy {
        id,
        pos: p,
        prev: p,
        hp: 1e6,
        kind: 0,
        clock: 0.0,
        direction: Vec2::ZERO,
        chipped: 0,
        body: Body::at(support(&g.terrain, p, g.physics.radius)),
    });
}
fn advance(g: &mut Game, input: Vec2, frames: usize) {
    for _ in 0..frames {
        g.frame(1.0 / 60.0, input);
        audit(g);
    }
}
pub fn audit(g: &mut Game) {
    let actors = std::iter::once((&g.player_body, g.player))
        .chain(g.enemies.iter().map(|e| (&e.body, e.pos)));
    for (b, p) in actors {
        if b.grounded
            && (b.y
                - crate::controller::ground(&g.terrain, &g.walls, p, g.physics.radius, b.y + 0.025))
            .abs()
                > 0.04
        {
            g.metrics.grounding_errors += 1;
        }
        if !clear(&g.terrain, &g.walls, p, b.y, &g.physics) {
            g.metrics.penetration_errors += 1;
        }
        if (b.y - b.prev_y).abs() > g.physics.step_height + 0.02 && b.grounded {
            g.metrics.vertical_spikes += 1;
        }
    }
}
/// Diagnostic only: a region must have at least one reachable point two metres away.
pub fn trapped(g: &Game) -> bool {
    if !g.player_body.grounded || g.navigation.distance.len() != COLS() * COLS() {
        return false;
    }
    !g.navigation.distance.iter().enumerate().any(|(i, &d)| {
        d != u32::MAX
            && g.terrain
                .world_center(i % COLS(), i / COLS())
                .distance_squared(g.player)
                > 4.0
    })
}
fn stairs(g: &mut Game, diagonal: bool) {
    for z in 0..COLS() {
        for x in 0..COLS() {
            let p = g.terrain.world_center(x, z);
            if p.abs().max_element() < 10.0 {
                let along = if diagonal {
                    (p.x + p.y) * 0.70710677
                } else {
                    p.x
                };
                let h = ((along + 4.0).max(0.0) / 1.0).floor().min(6.0) * 0.25;
                g.terrain.set_column(p, h);
            }
        }
    }
}
fn row(name: &str, work: impl FnOnce(&mut Game) -> Result<String, String>) -> ResultRow {
    let start = std::time::Instant::now();
    let mut g = fixture();
    let result = work(&mut g);
    let clean = g.metrics.grounding_errors == 0 && g.metrics.penetration_errors == 0;
    ResultRow {
        scenario: name.into(),
        passed: result.is_ok() && clean,
        detail: format!(
            "{}; grounding {}, penetration {}",
            result.unwrap_or_else(|e| e),
            g.metrics.grounding_errors,
            g.metrics.penetration_errors
        ),
        terrain_update_us: g.metrics.terrain_update_us,
        navigation_update_us: g.metrics.navigation_update_us,
        metrics: g.metrics,
        elapsed_ms: start.elapsed().as_secs_f64() * 1000.0,
    }
}
pub fn scenarios() -> Vec<ResultRow> {
    vec![
        row("staircase ascent/descent", |g| {
            stairs(g, false);
            place(g, Vec2::new(-6.0, 0.0));
            advance(g, Vec2::X, 180);
            let high = g.player_body.y;
            let end = g.player.x;
            advance(g, -Vec2::X, 180);
            if end > 3.0 && high >= 1.4 && g.player.x < -4.0 && g.player_body.y < 0.1 {
                Ok(format!(
                    "ascent x {end:.2}, y {high:.2}; descended to {:?}",
                    g.player
                ))
            } else {
                Err(format!(
                    "staircase blocked: x {end}, height {high}, return {:?}",
                    g.player
                ))
            }
        }),
        row("diagonal slope, stop/start, chunk seams", |g| {
            stairs(g, true);
            place(g, Vec2::splat(-4.5));
            let d = Vec2::ONE.normalize();
            for _ in 0..6 {
                advance(g, d, 45);
                advance(g, Vec2::ZERO, 30);
            }
            let h = g.player_body.y;
            if g.player.min_element() > 2.0 && h > 1.0 {
                Ok(format!("diagonal {:?}, y {h:.2}, stable pauses", g.player))
            } else {
                Err(format!("diagonal blocked {:?} y {h}", g.player))
            }
        }),
        row("grenade beneath player, minimum-time fall, escape", |g| {
            let initial = g.player_body.y;
            g.explode(Vec3::ZERO, 3.2, 0.6, 0.0);
            let crater = g.terrain.height(Vec2::ZERO);
            if crater >= -0.5 || g.player_body.grounded {
                return Err("explosion failed to remove support".into());
            }
            advance(g, Vec2::ZERO, 120);
            let falling = g.player_body.y;
            if !(falling < initial && falling > crater + 0.5 && g.world < 0.1) {
                return Err(format!(
                    "fall ignored slow clock: y {falling}, floor {crater}, world {}",
                    g.world
                ));
            }
            advance(g, Vec2::ZERO, 1500);
            let landed = g.player_body.grounded;
            advance(g, Vec2::X, 150);
            if landed && g.player.x > 5.0 && g.player_body.y > -0.3 {
                Ok(format!(
                    "floor {crater}, slow fall {falling:.3}, escaped {:?}",
                    g.player
                ))
            } else {
                Err(format!(
                    "escape failed landed {landed}, {:?}, y {}",
                    g.player, g.player_body.y
                ))
            }
        }),
        row("ordinary crater enter and leave", |g| {
            g.explode(Vec3::ZERO, 3.2, 0.6, 0.0);
            place(g, Vec2::new(-6.0, 0.0));
            advance(g, Vec2::X, 110);
            let low = g.player_body.y;
            advance(g, Vec2::X, 110);
            if low < -1.0 && g.player.x > 4.0 && g.player_body.y > -0.3 {
                Ok(format!("crossed bowl y {low:.2}, exit {:?}", g.player))
            } else {
                Err(format!(
                    "crater crossing {:?}, y {}, low {low}",
                    g.player, g.player_body.y
                ))
            }
        }),
        row("enemy group loses support and navigates crater exit", |g| {
            place(g, Vec2::new(7.0, 0.0));
            for i in 0..12 {
                enemy(
                    g,
                    Vec2::new((i % 4) as f32 * 0.3 - 0.5, (i / 4) as f32 * 0.3 - 0.4),
                );
            }
            g.explode(Vec3::ZERO, 3.2, 0.6, 0.0);
            if g.enemies.iter().any(|e| e.body.grounded) {
                return Err("enemies retained excavated support".into());
            }
            advance(g, Vec2::ZERO, 900);
            // Keep this local traversal fixture in the active streaming window.
            advance(g, Vec2::X, 180);
            let exited = g.enemies.iter().filter(|e| e.body.y > -0.5).count();
            if exited >= 10 {
                Ok(format!(
                    "{exited}/12 enemies walked out; nav rebuilds {}",
                    g.metrics.navigation_rebuilds
                ))
            } else {
                Err(format!(
                    "only {exited}/12 escaped; {:?}",
                    g.enemies
                        .iter()
                        .map(|e| (e.pos, e.body.y))
                        .collect::<Vec<_>>()
                ))
            }
        }),
        row("overlapping blasts, deep blocked pit, bedrock", |g| {
            for offset in [0.0, 1.0, -1.0, 0.5] {
                let p = Vec2::new(offset, 0.0);
                let y = g.terrain.height(p);
                g.explode(point(p, y), 3.2, 0.8, 0.0);
            }
            let ordinary = g.terrain.height(Vec2::ZERO);
            let pit = Vec2::splat(15.0);
            for _ in 0..6 {
                let y = g.terrain.height(pit);
                g.explode(point(pit, y), 1.2, 2.5, 0.0);
            }
            place(g, pit);
            enemy(g, pit + Vec2::X * 6.0);
            advance(g, Vec2::X, 600);
            if g.terrain.height(pit) >= crate::terrain::BOTTOM + crate::terrain::Y()
                && g.player.x < 17.0
                && g.metrics.unreachable_reports > 0
                && trapped(g)
            {
                Ok(format!(
                    "overlap floor {ordinary:.2}; honest trapped pit diagnosed {} times",
                    g.metrics.unreachable_reports
                ))
            } else {
                Err(format!(
                    "pit/boundary failed: {:?}, unreachable {}",
                    g.player, g.metrics.unreachable_reports
                ))
            }
        }),
        row(
            "camera rim orbit, partial body occlusion, real-time transitions",
            |g| {
                g.explode(Vec3::ZERO, 3.2, 0.6, 0.0);
                place(g, Vec2::ZERO);
                g.walls.push(crate::sim::Wall::new(
                    Vec3::new(-1.0, -2.0, 1.5),
                    [10, 25, 2],
                ));
                let before = g.world;
                let mut camera = crate::camera::Camera::default();
                for _ in 0..90 {
                    camera.update(g, g.player, 0.0, 1.0 / 60.0);
                }
                let hidden = camera.fade;
                for _ in 0..90 {
                    camera.update(g, g.player, std::f32::consts::PI, 1.0 / 60.0);
                }
                let clear = camera.fade;
                if hidden > 0.9 && clear < 0.01 && g.world == before {
                    Ok(format!(
                        "occluded fade {hidden:.3}, restored {clear:.3}; simulation unchanged"
                    ))
                } else {
                    Err(format!("camera transition {hidden} -> {clear}"))
                }
            },
        ),
        row("combat across elevations and terrain cover", |g| {
            g.explode(Vec3::ZERO, 3.2, 0.6, 0.0);
            place(g, Vec2::ZERO);
            enemy(g, Vec2::new(2.5, 0.0));
            enemy(g, Vec2::new(6.0, 0.0));
            g.walls
                .push(crate::sim::Wall::new(Vec3::new(4.0, 0.0, -0.8), [2, 15, 8]));
            let near = g.enemies[0].id;
            let covered = g.enemies[1].id;
            let visible = g.target(8.0) == Some(near);
            let cover = g.obstruction(g.player, g.enemies[1].pos).is_some();
            g.build.push(crate::shop::Owned {
                id: "needle".into(),
                level: 1,
                spent: 0,
            });
            g.enemies[0].kind = 2;
            advance(g, Vec2::ZERO, 1500);
            let hit = g
                .events
                .iter()
                .any(|e| e["kind"] == "hit" && e["data"]["target"] == near);
            if visible && cover && hit {
                Ok(format!(
                    "height projectile hit {near}, structure covered target {covered}"
                ))
            } else {
                Err(format!(
                    "visible {visible}, covered {cover}, projectile hit {hit}"
                ))
            }
        }),
        row(
            "automatic grenade, overlapping slow-motion/high-load explosions",
            |g| {
                g.build.push(crate::shop::Owned {
                    id: "grenade".into(),
                    level: 1,
                    spent: 0,
                });
                for i in 0..300 {
                    let a = i as f32 * 0.37;
                    enemy(
                        g,
                        Vec2::new(a.cos(), a.sin()) * (3.0 + (i % 8) as f32 * 0.45),
                    );
                }
                advance(g, Vec2::X, 90);
                let automatic = g.events.iter().any(|e| e["kind"] == "grenade");
                let edited = g.metrics.terrain_edits;
                for i in 0..360 {
                    if i % 30 == 0 {
                        let p = Vec2::new((i % 5) as f32 * 0.3, 0.0);
                        let y = g.terrain.height(p) + 0.3;
                        g.grenades.push(Grenade {
                            pos: point(p, y),
                            prev: point(p, y),
                            velocity: Vec3::ZERO,
                            fuse: 0.02,
                            damage: 0.0,
                            radius: 3.2,
                            strength: 0.6,
                        });
                    }
                    g.frame(1.0 / 60.0, Vec2::ZERO);
                    audit(g);
                }
                if automatic
                    && edited > 0
                    && g.metrics.terrain_edits > edited
                    && g.debris.len() <= g.config.debris_budget
                {
                    Ok(format!(
                        "automatic crater plus {} edits; 300-enemy slowed load, debris peak {}",
                        g.metrics.terrain_edits, g.metrics.debris_peak
                    ))
                } else {
                    Err(format!(
                        "automatic {automatic}, initial edits {edited}, total {}",
                        g.metrics.terrain_edits
                    ))
                }
            },
        ),
        row(
            "extensive excavation checkpoint, shop/wave retention and new run",
            |g| {
                for i in 0..18 {
                    let p = Vec2::new((i % 6) as f32 * 3.0 - 8.0, (i / 6) as f32 * 3.0 - 4.0);
                    g.explode(point(p, g.terrain.height(p)), 3.2, 0.6, 0.0);
                }
                place(g, Vec2::ZERO);
                let chunks = g
                    .terrain
                    .chunks
                    .iter()
                    .map(|c| c.checksum)
                    .collect::<Vec<_>>();
                g.phase = Phase::Shop;
                g.fill_shop();
                let path = std::path::Path::new("target/terrain-regression/checkpoint.json");
                g.save(path)?;
                g.save(path)?;
                let mut loaded = Game::load(path)?;
                loaded.start_wave()?;
                if chunks
                    == loaded
                        .terrain
                        .chunks
                        .iter()
                        .map(|c| c.checksum)
                        .collect::<Vec<_>>()
                    && loaded.terrain.validate()
                    && Game::new(42, Config::default()).terrain.height(Vec2::ZERO) == 0.0
                {
                    Ok(format!(
                        "{} edits, bit-exact save/load through shop to wave {}",
                        g.metrics.terrain_edits, loaded.wave
                    ))
                } else {
                    Err("checkpoint/reset mismatch".into())
                }
            },
        ),
    ]
}
pub fn run() -> Result<(), String> {
    let rows = scenarios();
    let dir = std::path::Path::new("reports/voxel-regression");
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let passed = rows.iter().filter(|r| r.passed).count();
    let report = serde_json::json!({"build":env!("HV_SOURCE_HASH"),"privileged_fixtures":true,"rows":rows,"passed":passed,"total":rows.len()});
    std::fs::write(
        dir.join("summary.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    let mut md = String::from(
        "# Voxel terrain regression\n\nPrivileged setup, normal controller/combat simulation. Headless only; rendered checks are separate.\n\n",
    );
    for r in &rows {
        md += &format!(
            "- {}: **{}** — {} ({:.1} ms)\n",
            r.scenario,
            if r.passed { "PASS" } else { "FAIL" },
            r.detail,
            r.elapsed_ms
        );
    }
    std::fs::write(dir.join("summary.md"), md).map_err(|e| e.to_string())?;
    println!(
        "Terrain regression: {passed}/{}; reports/voxel-regression/summary.json",
        rows.len()
    );
    if passed == rows.len() {
        Ok(())
    } else {
        Err("Terrain regression failures; inspect report".into())
    }
}
/// Rendered diagnostic: ordinary craters, then normal automatic combat and orbiting camera.
pub fn demo(g: &mut Game) {
    g.spawn = 1e6;
    g.config.contact_damage = 0.0;
    g.config.wave_seconds = 600.0;
    for p in [Vec2::ZERO, Vec2::new(2.0, 0.0), Vec2::new(-2.0, -1.0)] {
        g.explode(point(p, g.terrain.height(p)), 3.2, 0.6, 0.0);
    }
    place(g, Vec2::ZERO);
    for i in 0..20 {
        let a = i as f32 * 0.31;
        let p = Vec2::new(a.cos(), a.sin()) * 6.0;
        let y = support(&g.terrain, p, g.physics.radius);
        if !clear(&g.terrain, &g.walls, p, y, &g.physics) {
            continue;
        }
        enemy(g, p);
        g.enemies.last_mut().unwrap().hp = 150.0;
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn regression_scenarios() {
        let rows = super::scenarios();
        let errors: Vec<_> = rows
            .iter()
            .filter(|r| !r.passed)
            .map(|r| format!("{}: {}", r.scenario, r.detail))
            .collect();
        assert!(errors.is_empty(), "{}", errors.join("\n"));
    }
}
