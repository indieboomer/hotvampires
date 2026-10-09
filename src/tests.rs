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
    g.player = Vec2::new(14.2, 0.0);
    for _ in 0..120 {
        g.frame(1.0 / 60.0, Vec2::X);
    }
    assert!(g.scale > 0.99);
    assert!(g.player.x < 14.3);
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
