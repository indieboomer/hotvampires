use crate::{
    content::Config,
    shop::Offer,
    sim::{Game, Phase, Wall},
};
use glam::{Vec2, Vec3};

#[test]
#[ignore]
fn profile_rotation_and_ruin_fall() {
    let mut g = game();
    crate::world::ruin_demo(&mut g);
    let mut cache = crate::render::SceneCache::default();
    cache.scene(&g, 0.0, 10.0, 1.6);
    let mut frames = Vec::new();
    let mut physics = Vec::new();
    for frame in 0..180 {
        let start = std::time::Instant::now();
        g.settle_structures(crate::sim::STEP * 2.0);
        physics.push(start.elapsed().as_secs_f64() * 1000.0);
        std::hint::black_box(cache.scene(&g, frame as f32 * 0.035, 10.0, 1.6));
        frames.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    fn summary(mut samples: Vec<f64>) -> serde_json::Value {
        samples.sort_by(f64::total_cmp);
        serde_json::json!({"mean_ms":samples.iter().sum::<f64>()/samples.len() as f64,"p95_ms":samples[samples.len()*95/100],"peak_ms":samples.last()})
    }
    let report = serde_json::json!({"cpu_frame":summary(frames),"physics":summary(physics),"wall_mesh_rebuilds":cache.wall_mesh_rebuilds});
    std::fs::write(
        "target/rotation-fall-profile.json",
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    println!("{report}");
}

#[test]
fn terrain_support_cache_ignores_distant_edits_and_refreshes_removed_foundations() {
    let mut g = game();
    g.walls = vec![Wall::new(Vec3::new(4.0, 0.0, 4.0), [2, 5, 2])];
    g.support_revision = u64::MAX;
    g.settle_structures(crate::sim::STEP);
    let stamp = g.walls[0].support_terrain_stamp;
    assert!(stamp.is_some());
    g.terrain.set_column(Vec2::new(-20.0, -20.0), -2.0);
    g.settle_structures(crate::sim::STEP);
    assert_eq!(g.walls[0].support_terrain_stamp, stamp);
    assert!(!g.walls[0].falling);
    for z in 142..147 {
        for x in 142..147 {
            g.terrain
                .set_column(crate::terrain::Terrain::column_center(x, z), -2.0);
        }
    }
    g.settle_structures(crate::sim::STEP);
    assert!(g.walls.iter().any(|w| w.falling));
}

/// Repeatable CPU profile: falling voxel blocks among the streamed scenery.
#[test]
#[ignore]
fn profile_structural_physics() {
    let mut g = game();
    g.build.clear();
    for i in 0..16 {
        let mut w = Wall::new(
            Vec3::new(
                (i % 4) as f32 * 3.0 - 6.0,
                5.0 + (i % 3) as f32,
                (i / 4) as f32 * 3.0 - 6.0,
            ),
            [8, 8, 8],
        );
        w.falling = true;
        w.material = 3;
        g.walls.push(w);
    }
    let mut cache = crate::render::SceneCache::default();
    g.navigation
        .update(&g.terrain, &g.walls, &g.physics, g.player, crate::sim::STEP);
    cache.scene(&g, 0.4, 10.0, 1.6);
    let mut physics = Vec::new();
    let mut nav = Vec::new();
    let mut render = Vec::new();
    let mut frame = Vec::new();
    for _ in 0..120 {
        let start = std::time::Instant::now();
        let mut pu = 0.0;
        let mut nu = 0.0;
        for _ in 0..2 {
            let t = std::time::Instant::now();
            g.settle_structures(crate::sim::STEP);
            pu += t.elapsed().as_secs_f64() * 1000.0;
            let t = std::time::Instant::now();
            g.navigation
                .update(&g.terrain, &g.walls, &g.physics, g.player, crate::sim::STEP);
            nu += t.elapsed().as_secs_f64() * 1000.0;
        }
        let t = std::time::Instant::now();
        std::hint::black_box(cache.scene(&g, 0.4, 10.0, 1.6));
        render.push(t.elapsed().as_secs_f64() * 1000.0);
        physics.push(pu);
        nav.push(nu);
        frame.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    fn stats(mut values: Vec<f64>) -> serde_json::Value {
        values.sort_by(f64::total_cmp);
        serde_json::json!({"mean_ms":values.iter().sum::<f64>()/values.len() as f64,"p95_ms":values[values.len()*95/100],"peak_ms":values.last()})
    }
    let report = serde_json::json!({"physics":stats(physics),"navigation":stats(nav),"scene":stats(render),"cpu_frame":stats(frame),"navigation_rebuilds":g.navigation.rebuilds,"wall_mesh_rebuilds":cache.wall_mesh_rebuilds,"wall_mesh_translations":cache.wall_mesh_translations,"walls":g.walls.len(),"remaining_falling":g.walls.iter().filter(|w|w.falling).count()});
    std::fs::write(
        "target/physics-profile.json",
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    println!("{report}");
    assert_eq!(g.walls.iter().filter(|w| w.falling).count(), 0);
}

#[test]
fn falling_meshes_reuse_shape_and_refresh_for_real_shape_changes() {
    let mut g = game();
    let mut w = Wall::new(Vec3::new(3.0, 6.0, 3.0), [3, 3, 3]);
    w.falling = true;
    g.walls = vec![w];
    let mut cache = crate::render::SceneCache::default();
    cache.scene(&g, 0.4, 10.0, 1.6);
    let builds = cache.wall_mesh_rebuilds;
    for _ in 0..30 {
        g.settle_structures(crate::sim::STEP);
        cache.scene(&g, 0.4, 10.0, 1.6);
    }
    assert_eq!(cache.wall_mesh_rebuilds, builds);
    assert!(cache.wall_mesh_translations > 0);
    g.walls[0].cells[13] = false;
    cache.scene(&g, 0.4, 10.0, 1.6);
    assert_eq!(cache.wall_mesh_rebuilds, builds + 1);
    g.walls[0].material = 2;
    cache.scene(&g, 0.4, 10.0, 1.6);
    assert_eq!(cache.wall_mesh_rebuilds, builds + 2);
}

#[test]
fn cached_falling_contacts_refresh_after_terrain_is_removed() {
    let mut g = game();
    let p = Vec2::splat(3.0);
    for z in 139..143 {
        for x in 139..143 {
            g.terrain
                .set_column(crate::terrain::Terrain::column_center(x, z), 1.0);
        }
    }
    let mut w = Wall::new(Vec3::new(p.x, 3.0, p.y), [1, 1, 1]);
    w.falling = true;
    g.walls = vec![w];
    g.settle_structures(crate::sim::STEP);
    assert!(g.walls[0].contact_clearance.is_some());
    for z in 139..143 {
        for x in 139..143 {
            g.terrain
                .set_column(crate::terrain::Terrain::column_center(x, z), -1.0);
        }
    }
    for _ in 0..150 {
        g.settle_structures(crate::sim::STEP);
    }
    assert!(!g.walls[0].falling);
    assert!((g.walls[0].origin.y + 1.0).abs() < 0.00001);
}

#[test]
fn moving_navigation_is_bounded_and_landing_refreshes_immediately() {
    let mut g = game();
    let mut w = Wall::new(Vec3::new(3.0, 6.0, 3.0), [3, 3, 3]);
    w.falling = true;
    g.walls = vec![w];
    let mut nav = crate::navigation::Navigation::default();
    for _ in 0..30 {
        g.settle_structures(crate::sim::STEP);
        nav.update(&g.terrain, &g.walls, &g.physics, g.player, crate::sim::STEP);
    }
    assert!(nav.rebuilds <= 4);
    for _ in 0..180 {
        g.settle_structures(crate::sim::STEP);
        nav.update(&g.terrain, &g.walls, &g.physics, g.player, crate::sim::STEP);
    }
    assert!(!g.walls[0].falling);
    let builds = nav.rebuilds;
    nav.update(&g.terrain, &g.walls, &g.physics, g.player, crate::sim::STEP);
    assert_eq!(nav.rebuilds, builds);
}
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
    let center = Vec2::new(96.0, 64.0);
    let expected = crate::terrain::height(42, center);
    for offset in [
        Vec2::ZERO,
        Vec2::new(-4.0, 4.0),
        Vec2::new(4.0, -4.0),
        Vec2::splat(4.0),
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
        assert!(h.is_finite() && h.abs() <= 6.0);
        assert!((h - crate::terrain::height(42, p + Vec2::X * 0.002)).abs() < 0.01);
    }
}

#[test]
fn terrain_preserves_the_starting_building_foundations() {
    let g = game();
    for wall in g.walls.iter().filter(|w| w.tile.is_none()) {
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
        g.terrain.stream(pos);
        g.stream_props();
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
fn streamed_world_crosses_the_old_arena_edge_and_keeps_spawns_loaded() {
    let mut g = Game::new(
        42,
        Config {
            wave_seconds: 600.0,
            contact_damage: 0.0,
            ..Config::default()
        },
    );
    g.walls.clear();
    g.terrain.stream(g.player);
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
    assert!(g.player.x > 31.5);
    assert!(g.blocked(Vec2::splat(-10000.0), 0.3));
    assert!(g.enemies.iter().all(|e| g.terrain.bounds(e.pos, 0.4)));
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
        crate::terrain::BOTTOM + crate::terrain::Y()
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
    g.terrain.tops[64 + 64 * crate::terrain::COLS()] = 0;
    assert!(!g.terrain.validate());
    let path = std::path::Path::new("target/test-save/corrupted.json");
    g.save(path).unwrap();
    assert!(Game::load(path).is_err());
}
#[test]
fn smaller_voxels_preserve_world_dimensions_and_legacy_excavation() {
    use crate::terrain::{COLS, Chunk, LAYERS, Terrain, XZ, Y};
    assert_eq!(XZ(), 0.25);
    assert_eq!(Y(), 0.125);
    assert_eq!(COLS() as f32 * XZ(), 64.0);
    assert_eq!(LAYERS() as f32 * Y(), 16.0);
    let mut g = shop();
    let mut old = Terrain {
        origin: Vec2::ZERO,
        edits: Default::default(),
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
fn streamed_terrain_reuses_chunks_and_restores_excavation_after_travel() {
    let mut t = crate::terrain::Terrain::new(42);
    t.set_column(Vec2::ZERO, -2.0);
    let original = t.height(Vec2::ZERO);
    let checksum = t.chunks[4 + 4 * crate::terrain::CHUNKS()].checksum;
    assert!(t.stream(Vec2::new(9.0, 0.0)));
    assert_eq!(
        t.chunks[3 + 4 * crate::terrain::CHUNKS()].checksum,
        checksum
    );
    assert_eq!(t.height(Vec2::ZERO), original);
    for p in [Vec2::splat(10000.0), Vec2::splat(-10000.0), Vec2::ZERO] {
        t.stream(p);
        assert!(t.validate());
        assert_eq!(t.chunks.len(), 64);
        assert_eq!(t.tops.len(), 256 * 256);
    }
    assert_eq!(t.height(Vec2::ZERO), original);
}

#[test]
fn disconnected_wall_voxels_fall_as_solid_grid_pieces_and_stay_landed() {
    let mut g = game();
    g.walls = vec![Wall::new(Vec3::ZERO, [1, 6, 1])];
    g.walls[0].cells[0] = false;
    g.walls[0].cells[1] = false;
    let mut mesh = crate::render::SceneCache::default();
    mesh.scene(&g, 0.4, 10.0, 1.6);
    let mut airborne = false;
    for _ in 0..120 {
        g.settle_structures(crate::sim::STEP);
        airborne |= g.walls.iter().any(|w| w.falling);
    }
    assert!(airborne);
    assert_eq!(
        g.walls
            .iter()
            .map(|w| w.cells.iter().filter(|&&v| v).count())
            .sum::<usize>(),
        4
    );
    let piece = g.walls.iter().find(|w| w.cells.iter().any(|&v| v)).unwrap();
    assert!(!piece.falling);
    assert!(piece.origin.y.abs() < 0.00001);
    assert!(piece.solid3(Vec3::splat(0.1)));
    let before = g.walls.len();
    for _ in 0..120 {
        g.settle_structures(crate::sim::STEP);
    }
    assert_eq!(g.walls.len(), before);
    // Moving structure geometry must also invalidate the renderer's cache.
    let moved = mesh.scene(&g, 0.4, 10.0, 1.6);
    let fresh = crate::render::SceneCache::default().scene(&g, 0.4, 10.0, 1.6);
    assert_eq!(moved.len(), fresh.len());
}

#[test]
fn scenery_is_seeded_bounded_and_preserves_destroyed_props_on_revisit() {
    let mut g = game();
    let baseline = serde_json::to_vec(&g.walls).unwrap();
    assert_eq!(baseline, serde_json::to_vec(&game().walls).unwrap());
    assert!(g.walls.iter().any(|w| w.material == 1));
    assert!(g.walls.iter().any(|w| w.material == 2));
    assert!(g.walls.iter().any(|w| w.material == 3));
    let index = g.walls.iter().position(|w| w.material == 3).unwrap();
    let tile = g.walls[index].tile.clone();
    let origin = g.walls[index].origin;
    g.walls[index].cells.fill(false);
    g.walls[index].modified = true;
    for p in [Vec2::splat(10000.0), Vec2::splat(-10000.0), Vec2::ZERO] {
        g.terrain.stream(p);
        g.stream_props();
        assert!(g.walls.len() <= 80 + 25 * 16);
    }
    let rock = g
        .walls
        .iter()
        .find(|w| w.tile == tile && w.origin == origin)
        .unwrap();
    assert!(rock.cells.iter().all(|&v| !v));
    let cells = g
        .walls
        .iter()
        .map(|w| w.cells.iter().filter(|&&v| v).count())
        .sum::<usize>();
    for _ in 0..240 {
        g.settle_structures(crate::sim::STEP);
    }
    assert!(g.walls.len() <= 100 + 25 * 16);
    assert_eq!(
        g.walls
            .iter()
            .map(|w| w.cells.iter().filter(|&&v| v).count())
            .sum::<usize>(),
        cells
    );
}

#[test]
fn groundcover_is_walkthrough_damageable_and_does_not_invalidate_navigation() {
    let mut g = game();
    let mut bush = Wall::new(Vec3::new(4.0, 0.0, 4.0), [5, 4, 5]);
    bush.material = 6;
    let target = bush.center(32);
    let p = Vec2::new(target.x, target.z);
    g.walls = vec![bush];
    assert!(!g.blocked(p, g.physics.radius));
    assert!(crate::controller::clear(
        &g.terrain,
        &g.walls,
        p,
        g.terrain.height(p),
        &g.physics
    ));
    assert_eq!(
        crate::controller::ground(&g.terrain, &g.walls, p, g.physics.radius, 2.0),
        crate::controller::support(&g.terrain, p, g.physics.radius)
    );
    assert_eq!(
        g.ray3(target - Vec3::X * 1.0, target + Vec3::X * 1.0)
            .unwrap()
            .0,
        0
    );
    assert!(g.cover_ray3(target - Vec3::X, target + Vec3::X).is_none());
    g.navigation
        .update(&g.terrain, &g.walls, &g.physics, g.player, crate::sim::STEP);
    let rebuilds = g.navigation.rebuilds;
    let before = g.walls[0].cells.iter().filter(|&&c| c).count();
    g.chip_wall_at(0, target, 0.32);
    assert!(g.walls[0].cells.iter().filter(|&&c| c).count() < before);
    g.navigation
        .update(&g.terrain, &g.walls, &g.physics, g.player, crate::sim::STEP);
    assert_eq!(g.navigation.rebuilds, rebuilds);
    for _ in 0..10 {
        g.settle_structures(crate::sim::STEP);
    }
    assert_eq!(g.walls.len(), 1);
    assert!(!g.walls[0].falling);
}

#[test]
fn friendly_and_enemy_projectiles_and_explosions_destroy_soft_vegetation() {
    for material in [5, 6] {
        for hostile in [false, true] {
            let mut g = game();
            g.config.min_scale = 1.0;
            g.scale = 1.0;
            g.build.clear();
            g.enemies.clear();
            g.spawn = 1_000_000.0;
            let mut plant = Wall::new(Vec3::new(4.0, 0.0, 4.0), [3, 3, 3]);
            plant.material = material;
            g.walls = vec![plant];
            let p = Vec2::new(3.4, 4.3);
            g.shots.push(crate::sim::Shot {
                pos: p,
                prev: p,
                y: 0.5,
                prev_y: 0.5,
                velocity_y: 0.0,
                velocity: Vec2::new(100.0, 0.0),
                life: 1.0,
                damage: 5.0,
                hostile,
                pierce: 0,
                hit: Vec::new(),
                primary: true,
            });
            g.frame(crate::sim::STEP * 2.0, Vec2::X);
            assert!(
                g.walls[0].modified,
                "material {material}, hostile {hostile}"
            );
            assert!(g.walls[0].cells.iter().any(|&c| !c));
            let remaining = g.walls[0].cells.iter().filter(|&&c| c).count();
            g.explode(Vec3::new(4.3, 0.3, 4.3), 2.0, 1.0, 5.0);
            assert!(g.walls[0].cells.iter().filter(|&&c| c).count() < remaining);
        }
    }
}

#[test]
fn severed_grass_and_bush_voxels_fall_and_stack_without_actor_collision() {
    for material in [5, 6] {
        let mut g = game();
        let mut plant = Wall::new(Vec3::new(4.0, 0.0, 4.0), [1, 3, 1]);
        plant.material = material;
        let cut = plant.center(1);
        g.walls = vec![plant];
        g.chip_wall_at(0, cut, 0.11);
        g.settle_structures(crate::sim::STEP);
        assert_eq!(g.walls.len(), 2);
        assert!(g.walls[1].falling);
        assert_eq!(g.walls[1].material, material);
        let mut cache = crate::render::SceneCache::default();
        cache.scene(&g, 0.4, 10.0, 1.6);
        for _ in 0..120 {
            g.settle_structures(crate::sim::STEP);
        }
        assert!(!g.walls[1].falling);
        assert!((g.walls[1].origin.y - crate::sim::CELL).abs() < 0.0001);
        assert_eq!(
            g.walls
                .iter()
                .flat_map(|w| &w.cells)
                .filter(|&&c| c)
                .count(),
            2
        );
        let p = Vec2::new(4.1, 4.1);
        assert!(!g.blocked(p, g.physics.radius));
        assert!(crate::controller::clear(
            &g.terrain,
            &g.walls,
            p,
            g.terrain.height(p),
            &g.physics
        ));
        assert!(
            g.ray3(Vec3::new(3.5, 0.3, 4.1), Vec3::new(4.5, 0.3, 4.1))
                .is_some()
        );
        let moved = cache.scene(&g, 0.4, 10.0, 1.6);
        let fresh = crate::render::SceneCache::default().scene(&g, 0.4, 10.0, 1.6);
        assert_eq!(moved.len(), fresh.len());
    }
}

#[test]
fn tall_weakened_walls_topple_sideways_preserve_stone_and_damage_after_checkpoint() {
    for material in [0, 7] {
        let mut g = game();
        let mut wall = Wall::new(Vec3::new(4.0, 0.0, 4.0), [12, 24, 2]);
        wall.material = material;
        for z in 0..2 {
            for x in 0..12 {
                if x != 0 || z != 0 {
                    wall.cells[x + 12 * (2 + 24 * z)] = false;
                }
            }
        }
        g.walls = vec![wall];
        g.support_revision = u64::MAX;
        let rng = g.combat_rng.0;
        g.settle_structures(crate::sim::STEP);
        let wi = g
            .walls
            .iter()
            .position(|w| w.tree_topple.is_some())
            .unwrap();
        let topple = g.walls[wi].tree_topple.as_ref().unwrap();
        assert_eq!(topple.direction[0], 0);
        assert!(topple.voxels.iter().all(|v| v.material == 7));
        assert_eq!(g.combat_rng.0, rng);
        let count = g.walls[wi].cells.iter().filter(|&&c| c).count();
        let i = g.walls[wi].cells.iter().position(|&c| c).unwrap();
        let p = g.walls[wi].center(i);
        g.chip_wall_at(wi, p, 0.11);
        g.phase = Phase::Shop;
        let path = std::env::temp_dir().join(format!("hot-vampires-ruin-topple-{material}.json"));
        g.save(&path).unwrap();
        let mut loaded = Game::load(&path).unwrap();
        std::fs::remove_file(path).unwrap();
        for _ in 0..600 {
            loaded.settle_structures(crate::sim::STEP);
        }
        let fallen = &loaded.walls[wi];
        assert!(!fallen.falling && fallen.tree_topple.is_none());
        assert_eq!(fallen.cells.iter().filter(|&&c| c).count(), count - 1);
        assert!(fallen.size[2] > 2 && fallen.size[1] < 24);
        assert!(fallen.voxel_materials.iter().all(|&m| m == 0 || m == 7));
    }
}

#[test]
fn generated_tall_ruins_are_stable_and_damage_persists_after_streaming() {
    let mut g = game();
    let index = g.walls.iter().position(|w| w.material == 7).unwrap();
    let origin = g.walls[index].origin;
    let tile = g.walls[index].tile.clone();
    assert!(g.walls[index].size[1] >= 18);
    assert!(g.walls[index].cells.iter().any(|&c| !c));
    for _ in 0..30 {
        g.settle_structures(crate::sim::STEP);
    }
    assert!(!g.walls[index].falling && !g.walls[index].tree_felled);
    let center = origin + Vec3::splat(0.1);
    g.chip_wall_at(index, center, 0.3);
    let cells = g.walls[index].cells.clone();
    for p in [Vec2::splat(1000.0), Vec2::ZERO] {
        g.terrain.stream(p);
        g.stream_props();
    }
    assert_eq!(
        g.walls
            .iter()
            .find(|w| w.origin == origin && w.tile == tile && w.material == 7)
            .unwrap()
            .cells,
        cells
    );
}

#[test]
fn excavated_vegetation_falls_to_soil_and_can_land_on_solid_structures() {
    for material in [5, 6] {
        let mut g = game();
        let mut plant = Wall::new(Vec3::new(4.0, 0.0, 4.0), [1, 3, 1]);
        plant.material = material;
        g.walls = vec![plant];
        for z in 142..147 {
            for x in 142..147 {
                g.terrain
                    .set_column(crate::terrain::Terrain::column_center(x, z), -1.0);
            }
        }
        g.settle_structures(crate::sim::STEP);
        assert!(g.walls.iter().any(|w| w.falling));
        for _ in 0..120 {
            g.settle_structures(crate::sim::STEP);
        }
        let landed = g.walls.iter().find(|w| w.cells.iter().any(|&c| c)).unwrap();
        assert!(!landed.falling);
        assert!((landed.origin.y + 1.0).abs() < 0.0001);
        let mut raised = Wall::new(Vec3::new(4.0, 2.0, 4.0), [1, 2, 1]);
        raised.material = material;
        g.walls = vec![Wall::new(Vec3::new(4.0, -1.0, 4.0), [1, 5, 1]), raised];
        g.support_revision = u64::MAX;
        for _ in 0..120 {
            g.settle_structures(crate::sim::STEP);
        }
        let landed = g
            .walls
            .iter()
            .find(|w| w.material == material && w.cells.iter().any(|&c| c))
            .unwrap();
        assert!(!landed.falling);
        assert!(landed.origin.y.abs() < 0.0001);
    }
}

#[test]
fn generated_grass_and_bush_damage_survives_streaming_and_checkpoint() {
    let mut g = game();
    assert!(g.walls.iter().any(|w| w.material == 5));
    assert!(g.walls.iter().any(|w| w.material == 6));
    let wi = g.walls.iter().position(|w| w.material == 6).unwrap();
    let origin = g.walls[wi].origin;
    let tile = g.walls[wi].tile.clone();
    let center = origin + Vec3::new(0.5, 0.3, 0.5);
    g.chip_wall_at(wi, center, 2.0);
    let damaged = g.walls[wi].cells.clone();
    for p in [Vec2::splat(1000.0), Vec2::ZERO] {
        g.terrain.stream(p);
        g.stream_props();
    }
    let plant = g
        .walls
        .iter()
        .find(|w| w.origin == origin && w.tile == tile)
        .unwrap();
    assert_eq!(plant.cells, damaged);
    let path = std::env::temp_dir().join("hot-vampires-groundcover-checkpoint.json");
    g.phase = Phase::Shop;
    g.save(&path).unwrap();
    let loaded = Game::load(&path).unwrap();
    assert!(loaded.walls.iter().any(|w| w.material == 5));
    assert_eq!(
        loaded
            .walls
            .iter()
            .find(|w| w.origin == origin && w.tile == tile)
            .unwrap()
            .cells,
        damaged
    );
    std::fs::remove_file(path).unwrap();
}

#[test]
fn streamed_world_checkpoint_preserves_origin_edits_and_falling_pieces() {
    let mut g = game();
    let p = Vec2::new(-120.0, 240.0);
    g.terrain.stream(p);
    g.stream_props();
    g.player = p;
    g.prev_player = p;
    g.player_body = crate::controller::Body::at(g.terrain.height(p));
    g.terrain.set_column(p, -2.0);
    let mut piece = Wall::new(Vec3::new(p.x, 4.0, p.y), [2, 2, 2]);
    piece.falling = true;
    piece.fall_speed = 1.0;
    g.walls.push(piece);
    g.phase = Phase::Shop;
    let path = std::env::temp_dir().join("hot-vampires-streamed-checkpoint.json");
    g.save(&path).unwrap();
    let mut loaded = Game::load(&path).unwrap();
    assert_eq!(loaded.terrain.origin, g.terrain.origin);
    assert!(loaded.walls.last().unwrap().falling);
    loaded.terrain.stream(Vec2::ZERO);
    loaded.terrain.stream(p);
    assert_eq!(loaded.terrain.height(p), -2.0);
    std::fs::remove_file(path).unwrap();
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
