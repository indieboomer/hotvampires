use crate::{
    content::Config,
    shop::Offer,
    sim::{Game, Phase, Wall},
};
use glam::{Vec2, Vec3};
fn game() -> Game {
    Game::new(42, Config::default())
}
fn shop() -> Game {
    let mut g = game();
    g.phase = Phase::Shop;
    g.currency = 500;
    g.fill_shop();
    g
}
fn offer(g: &mut Game, id: &str) {
    g.shop.offers[0] = Offer {
        id: Some(id.into()),
        locked: false,
    };
}
#[test]
fn standing_never_stops_time() {
    let mut g = game();
    for _ in 0..600 {
        g.frame(1.0 / 60.0, Vec2::ZERO);
    }
    assert!((g.world - 0.3).abs() < 0.02);
    assert!(g.scale > 0.0);
}
#[test]
fn movement_uses_shared_clock_and_normalized_input() {
    let mut a = game();
    let mut b = game();
    a.walls.clear();
    b.walls.clear();
    for _ in 0..60 {
        a.frame(1.0 / 60.0, Vec2::X);
        b.frame(1.0 / 60.0, Vec2::new(1.0, 1.0));
    }
    assert!((a.player.length() - b.player.length()).abs() < 0.01);
    assert!(a.scale > 0.99);
}
#[test]
fn blocked_attempt_restores_speed() {
    let mut g = game();
    g.player = Vec2::new(0.0, 2.69);
    for _ in 0..120 {
        g.frame(1.0 / 60.0, Vec2::Y);
    }
    assert!(g.scale > 0.99);
    assert!(g.player.y < 2.71);
}
#[test]
fn shop_fully_pauses() {
    let mut g = shop();
    let before = g.world;
    g.frame(1.0, Vec2::X);
    assert_eq!(g.world, before);
}
#[test]
fn duplicate_purchase_is_atomic() {
    let mut g = shop();
    offer(&mut g, "capacitor");
    g.buy(0).unwrap();
    let coins = g.currency;
    assert!(g.buy(0).is_err());
    assert_eq!(g.currency, coins);
    assert_eq!(g.level("capacitor"), 1);
}
#[test]
fn selling_recomputes_and_refund_cannot_arbitrage() {
    let mut g = shop();
    offer(&mut g, "capacitor");
    let base = g.stats().damage;
    g.buy(0).unwrap();
    assert!(g.stats().damage > base);
    g.sell("capacitor").unwrap();
    assert_eq!(g.stats().damage, base);
    assert_eq!(g.currency, 485);
}
#[test]
fn hp_items_do_not_heal_and_selling_clamps() {
    let mut g = shop();
    offer(&mut g, "core");
    g.buy(0).unwrap();
    assert_eq!(g.hp, 65.0);
    g.hp = 80.0;
    g.sell("core").unwrap();
    assert_eq!(g.hp, 65.0);
}
#[test]
fn locks_survive_reroll_and_visit() {
    let mut g = shop();
    g.lock(1).unwrap();
    let id = g.shop.offers[1].id.clone();
    g.reroll().unwrap();
    assert_eq!(g.shop.offers[1].id, id);
    assert_eq!(g.reroll_price(), 10);
    g.shop.rerolls = 0;
    g.fill_shop();
    assert_eq!(g.shop.offers[1].id, id);
    assert_eq!(g.reroll_price(), 5);
}
#[test]
fn max_level_and_funds_guard() {
    let mut g = shop();
    for _ in 0..3 {
        offer(&mut g, "pulse");
        g.buy(0).unwrap();
    }
    offer(&mut g, "pulse");
    assert!(g.buy(0).is_err());
    offer(&mut g, "ring");
    g.currency = 0;
    assert!(g.buy(0).is_err());
}
#[test]
fn full_skill_slots_still_allow_upgrades() {
    let mut g = shop();
    for id in ["echo", "rupture", "inertia", "momentum"] {
        offer(&mut g, id);
        g.buy(0).unwrap();
    }
    offer(&mut g, "chain");
    assert!(g.buy(0).is_err());
    offer(&mut g, "echo");
    assert!(g.buy(0).is_ok());
}
#[test]
fn destruction_changes_occupancy_and_budget() {
    let mut g = game();
    g.config.debris_budget = 8;
    let before = g.walls[0].cells.iter().filter(|v| **v).count();
    g.chip_wall(0, Vec2::new(0.0, 3.2), 0.8);
    assert!(g.walls[0].cells.iter().filter(|v| **v).count() < before);
    assert!(g.debris.len() <= 8);
    assert!(!g.walls[0].solid(Vec2::new(0.0, 3.2), 0.25));
}
#[test]
fn unsupported_sections_collapse() {
    let mut w = Wall::new(Vec3::ZERO, [1, 3, 1]);
    w.cells[1] = false;
    assert_eq!(w.support().len(), 1);
    assert!(w.cells[0]);
    assert!(!w.cells[2]);
}
#[test]
fn cover_blocks_world_target() {
    let mut g = game();
    g.enemies.push(crate::sim::Enemy {
        body: crate::controller::Body::at(0.0),
        id: 1,
        pos: Vec2::new(0.0, 5.0),
        prev: Vec2::new(0.0, 5.0),
        hp: 20.0,
        kind: 0,
        clock: 0.0,
        direction: Vec2::ZERO,
        chipped: 0,
    });
    assert_eq!(g.target(8.0), None);
    g.chip_wall(0, Vec2::new(0.0, 3.2), 0.8);
    assert_eq!(g.target(8.0), Some(1));
}
#[test]
fn reproducible_headless_clock_and_combat() {
    let mut a = game();
    let mut b = game();
    for i in 0..3600 {
        let v = Vec2::new((i as f32 * 0.003).cos(), (i as f32 * 0.003).sin());
        a.frame(1.0 / 60.0, v);
        b.frame(1.0 / 60.0, v);
    }
    assert_eq!(
        serde_json::to_string(&a).unwrap(),
        serde_json::to_string(&b).unwrap()
    );
}
#[test]
fn checkpoint_preserves_shop_and_breach() {
    let mut g = shop();
    g.chip_wall(0, Vec2::new(0.0, 3.2), 0.8);
    let dir = std::path::Path::new("target/test-save");
    std::fs::create_dir_all(dir).unwrap();
    let path = dir.join("checkpoint.json");
    g.save(&path).unwrap();
    g.save(&path).unwrap();
    let loaded = Game::load(&path).unwrap();
    assert_eq!(
        serde_json::to_string(&g.shop).unwrap(),
        serde_json::to_string(&loaded.shop).unwrap()
    );
    assert_eq!(g.walls[0].cells, loaded.walls[0].cells);
    assert_eq!(g.shop_rng.0, loaded.shop_rng.0);
}

#[test]
fn camera_mesh_generation_never_changes_world_or_target() {
    let g = game();
    let before = serde_json::to_string(&g).unwrap();
    let target = g.target(8.0);
    let mut cache = crate::render::SceneCache::default();
    for yaw in [0.0, 1.5, 3.0, 4.5, std::f32::consts::TAU] {
        assert!(!cache.scene(&g, yaw, 10.0, 1.6).is_empty());
        assert_eq!(g.target(8.0), target);
    }
    assert_eq!(serde_json::to_string(&g).unwrap(), before);
}
#[test]
fn spatial_grid_includes_swept_path_and_excludes_distant_entities() {
    let grid = crate::spatial::Grid::new(
        [
            (0, Vec2::ZERO),
            (1, Vec2::new(3.0, 0.0)),
            (2, Vec2::splat(10.0)),
        ]
        .into_iter(),
    );
    assert_eq!(
        grid.segment(Vec2::new(-1.0, 0.0), Vec2::new(4.0, 0.0), 0.4),
        vec![0, 1]
    );
}

#[test]
fn terrain_has_flat_building_patches_and_seeded_variation() {
    let center = Vec2::new(96.0, 48.0);
    let expected = crate::terrain::height(42, center);
    for offset in [
        Vec2::ZERO,
        Vec2::new(-10.0, 10.0),
        Vec2::new(10.0, -10.0),
        Vec2::splat(10.0),
    ] {
        assert!((crate::terrain::height(42, center + offset) - expected).abs() < 0.00001);
    }
    assert_ne!(
        crate::terrain::height(42, center),
        crate::terrain::height(43, center)
    );
    for x in [-1000.5, -16.001, -16.0, -15.999, 0.0, 16.0, 999.5] {
        let p = Vec2::new(x, 37.3);
        let h = crate::terrain::height(42, p);
        assert!(h.is_finite() && h.abs() <= 4.3);
        assert!((h - crate::terrain::height(42, p + Vec2::X * 0.002)).abs() < 0.01);
    }
}

#[test]
fn terrain_preserves_the_starting_building_foundations() {
    let g = game();
    for wall in &g.walls {
        for i in 0..wall.cells.len() {
            let c = wall.center(i);
            if wall.coord(i)[1] == 0 {
                assert_eq!(
                    crate::terrain::height(g.seed, Vec2::new(c.x, c.z)),
                    wall.origin.y
                );
            }
        }
    }
}

#[test]
fn terrain_cache_is_bounded_and_generates_at_distant_negative_coordinates() {
    let mut g = game();
    let mut cache = crate::render::SceneCache::default();
    let mut count = 0;
    for pos in [
        Vec2::ZERO,
        Vec2::splat(10000.0),
        Vec2::splat(-10000.0),
        Vec2::new(-500.0, 1000.0),
    ] {
        g.player = pos;
        g.prev_player = pos;
        assert!(!cache.scene(&g, 0.4, 10.0, 1.6).is_empty());
        if count == 0 {
            count = cache.ground_chunks();
        } else {
            assert_eq!(cache.ground_chunks(), count);
        }
    }
    assert!(count <= 121);
}

#[test]
fn finite_arena_edges_stop_characters_and_spawns_stay_bounded() {
    let mut g = Game::new(
        42,
        Config {
            wave_seconds: 600.0,
            contact_damage: 0.0,
            ..Config::default()
        },
    );
    g.walls.clear();
    g.player = Vec2::new(28.0, 0.0);
    g.prev_player = g.player;
    g.player_body = crate::controller::Body::at(crate::controller::support(
        &g.terrain,
        g.player,
        g.physics.radius,
    ));
    for _ in 0..180 {
        g.frame(1.0 / 60.0, Vec2::X);
    }
    assert!(g.player.x < 31.5);
    assert!(g.player.x > 29.0);
    assert!(g.blocked(Vec2::splat(-10000.0), 0.3));
    assert!(
        g.enemies
            .iter()
            .all(|e| e.pos.abs().max_element() < crate::terrain::HALF)
    );
}

#[test]
fn excavation_rebuilds_only_dirty_meshes_and_changes_real_cover() {
    let mut g = game();
    g.walls.clear();
    let mut cache = crate::render::SceneCache::default();
    cache.scene(&g, 0.0, 10.0, 1.6);
    assert!(cache.synchronized(&g));
    let built = cache.mesh_rebuilds;
    g.explode(Vec3::ZERO, 1.2, 2.5, 0.0);
    assert!(!g.terrain.solid(Vec3::new(0.0, -0.5, 0.0)));
    assert!(!cache.synchronized(&g));
    assert!(
        g.ray3(Vec3::new(0.0, -2.0, 0.0), Vec3::new(5.0, 0.8, 0.0))
            .is_some()
    );
    cache.scene(&g, 1.0, 10.0, 1.6);
    assert!(cache.synchronized(&g));
    let changed = cache.mesh_rebuilds - built;
    assert!(changed > 0 && changed <= 16);
    cache.scene(&g, 2.0, 10.0, 1.6);
    assert_eq!(cache.mesh_rebuilds, built + changed);
    for _ in 0..8 {
        let y = g.terrain.height(Vec2::ZERO);
        g.explode(Vec3::new(0.0, y, 0.0), 1.2, 2.5, 0.0);
    }
    assert_eq!(
        g.terrain.height(Vec2::ZERO),
        crate::terrain::BOTTOM + crate::terrain::Y
    );
    assert!(g.terrain.validate());
    let border = g.terrain.top(0, 64);
    g.explode(Vec3::new(-32.0, 0.0, 0.0), 8.0, 3.0, 0.0);
    assert_eq!(g.terrain.top(0, 64), border);
}
#[test]
fn large_ledge_blocks_and_rounded_controller_slides_without_tunneling() {
    let mut g = game();
    g.walls.clear();
    g.spawn = 1e6;
    g.build.clear();
    for z in 116..140 {
        for x in 132..144 {
            g.terrain
                .set_column(crate::terrain::Terrain::column_center(x, z), 2.0);
        }
    }
    for _ in 0..60 {
        g.frame(1.0 / 60.0, Vec2::ONE);
    }
    assert!(g.player.x < 1.0);
    assert!(g.player.y > 2.0);
    assert!(crate::controller::clear(
        &g.terrain,
        &g.walls,
        g.player,
        g.player_body.y,
        &g.physics
    ));
}
#[test]
fn falling_lands_on_a_surviving_structure_in_world_time() {
    let mut g = game();
    g.walls = vec![Wall::new(Vec3::ZERO, [4, 4, 4])];
    g.player = Vec2::splat(0.4);
    g.prev_player = g.player;
    g.player_body = crate::controller::Body::at(2.5);
    g.player_body.grounded = false;
    g.spawn = 1e6;
    g.build.clear();
    for _ in 0..2000 {
        g.frame(1.0 / 60.0, Vec2::ZERO);
    }
    assert!(g.player_body.grounded);
    assert!((g.player_body.y - 0.8).abs() < 0.02);
}
#[test]
fn corrupted_voxel_checkpoint_is_rejected() {
    let mut g = shop();
    g.terrain.tops[64 + 64 * crate::terrain::COLS] = 0;
    assert!(!g.terrain.validate());
    let path = std::path::Path::new("target/test-save/corrupted.json");
    g.save(path).unwrap();
    assert!(Game::load(path).is_err());
}
#[test]
fn smaller_voxels_preserve_world_dimensions_and_legacy_excavation() {
    use crate::terrain::{COLS, Chunk, LAYERS, Terrain, XZ, Y};
    assert_eq!(XZ, 0.25);
    assert_eq!(Y, 0.125);
    assert_eq!(COLS as f32 * XZ, 64.0);
    assert_eq!(LAYERS as f32 * Y, 16.0);
    let mut g = shop();
    let mut old = Terrain {
        seed: g.seed,
        revision: 7,
        chunks: vec![
            Chunk {
                bits: vec![0; 2048],
                checksum: 0
            };
            64
        ],
        tops: vec![31; 128 * 128],
    };
    for z in 0..128 {
        for x in 0..128 {
            let top = if x == 0 || z == 0 || x == 127 || z == 127 {
                41
            } else if (60..68).contains(&x) && (60..68).contains(&z) {
                23
            } else {
                31
            };
            old.tops[x + 128 * z] = top;
            for y in 0..=top as usize {
                let bit = x % 16 + 16 * (z % 16 + 16 * y);
                old.chunks[x / 16 + 8 * (z / 16)].bits[bit / 8] |= 1 << (bit % 8);
            }
        }
    }
    for chunk in &mut old.chunks {
        chunk.checksum = chunk.bits.iter().fold(0xcbf29ce484222325u64, |h, &b| {
            (h ^ b as u64).wrapping_mul(0x100000001b3)
        });
    }
    g.terrain = old;
    g.player_body = crate::controller::Body::at(-2.0);
    let path = std::path::Path::new("target/test-save/coarse.json");
    g.save(path).unwrap();
    let loaded = Game::load(path).unwrap();
    assert!(loaded.terrain.validate());
    assert_eq!(loaded.terrain.revision, 7);
    assert_eq!(loaded.terrain.height(Vec2::ZERO), -2.0);
    assert_eq!(loaded.terrain.height(Vec2::splat(4.0)), 0.0);
    assert_eq!(loaded.terrain.height(Vec2::new(-31.75, 0.0)), 2.5);
}
#[test]
fn quarter_metre_stair_has_continuous_physical_capsule_contact() {
    let mut g = game();
    g.walls.clear();
    g.spawn = 1e6;
    g.build.clear();
    for z in 120..136 {
        for x in 132..148 {
            g.terrain
                .set_column(crate::terrain::Terrain::column_center(x, z), 0.25);
        }
    }
    let mut largest = 0.0f32;
    for _ in 0..60 {
        let old = g.player_body.y;
        g.frame(1.0 / 120.0, Vec2::X);
        largest = largest.max((g.player_body.y - old).abs());
    }
    assert!(g.player.x > 1.1);
    assert!((g.player_body.y - 0.25).abs() < 0.01);
    assert!(largest < 0.16, "physical stair rise {largest}");
}
#[test]
fn capsule_corner_contact_on_a_small_structure_is_stably_grounded() {
    let mut g = game();
    g.walls = vec![Wall::new(Vec3::new(1.0, 0.0, -0.5), [8, 2, 5])];
    g.spawn = 1e6;
    g.build.clear();
    for _ in 0..90 {
        g.frame(1.0 / 120.0, Vec2::X);
        crate::terrain_regression::audit(&mut g);
    }
    assert!(g.player.x > 2.0);
    assert_eq!(g.metrics.grounding_errors, 0);
    assert_eq!(g.metrics.penetration_errors, 0);
}
