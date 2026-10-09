//! A tree is one rigid voxel body while toppling; each visible pose occupies grid cells.
use crate::sim::{CELL, Game, Wall};
use glam::{IVec3, Vec2, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const POSE_STEP: f32 = std::f32::consts::FRAC_PI_2 / 16.0;

/// A reproducible native visual check, using an actual generated tree.
pub fn demo(g: &mut Game) {
    let wi = g
        .walls
        .iter()
        .enumerate()
        .filter(|(_, w)| w.material == 1)
        .min_by(|(_, a), (_, b)| {
            a.origin
                .length_squared()
                .total_cmp(&b.origin.length_squared())
        })
        .map(|(i, _)| i)
        .unwrap();
    let w = &mut g.walls[wi];
    let p = Vec2::new(w.origin.x, w.origin.z);
    for z in 0..w.size[2] {
        for x in 0..w.size[0] {
            if x != 0 || z != 0 {
                w.cells[x + w.size[0] * (2 + w.size[1] * z)] = false;
            }
        }
    }
    w.modified = true;
    w.revision += 1;
    g.invalidate_support_near(wi);
    g.player = p + Vec2::new(1.5, 2.5);
    g.prev_player = g.player;
    g.player_body = crate::controller::Body::at(g.terrain.height(g.player));
    g.config.min_scale = 1.0;
    g.scale = 1.0;
    g.config.wave_seconds = 600.0;
    g.spawn = 1_000_000.0;
    g.build.clear();
    g.enemies.clear();
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TreeVoxel {
    pub cell: [i32; 3],
    /// Zero denotes a voxel destroyed during the fall.
    pub material: u8,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn tree(seed: u64, supports: usize, cut: usize) -> Game {
        let mut g = Game::new(seed, crate::content::Config::default());
        let mut trunk = Wall::new(Vec3::new(3.0, 0.0, 3.0), [2, 14, 2]);
        trunk.material = 1;
        let mut removed = 0;
        for z in 0..2 {
            for x in 0..2 {
                if removed < 4 - supports {
                    trunk.cells[x + 2 * (cut + 14 * z)] = false;
                    removed += 1;
                }
            }
        }
        let mut crown = Wall::new(Vec3::new(2.4, 2.4, 2.4), [8, 8, 8]);
        crown.material = 2;
        for i in 0..crown.cells.len() {
            let c = crown.coord(i);
            crown.cells[i] = Vec3::new(
                c[0] as f32 + 0.5 - 4.0,
                c[1] as f32 + 0.5 - 4.0,
                c[2] as f32 + 0.5 - 4.0,
            )
            .length_squared()
                < 16.0;
        }
        g.walls = vec![trunk, crown];
        g
    }

    #[test]
    fn single_voxel_trunk_hinge_topples_upper_wood_and_canopy_together() {
        let mut g = tree(42, 1, 3);
        g.settle_structures(crate::sim::STEP);
        let wi = g
            .walls
            .iter()
            .position(|w| w.tree_topple.is_some())
            .unwrap();
        let topple = g.walls[wi].tree_topple.as_ref().unwrap();
        let direction = Vec2::new(topple.direction[0] as f32, topple.direction[1] as f32);
        let pivot = topple.pivot;
        let initial = g.walls[wi].cells.iter().filter(|&&v| v).count();
        assert!(g.walls[0].tree_felled);
        assert!(g.walls[0].cells.iter().any(|&v| v));
        assert!(g.walls[1].cells.iter().all(|&v| !v));
        assert!(g.walls[wi].voxel_materials.contains(&1));
        assert!(g.walls[wi].voxel_materials.contains(&2));
        for _ in 0..600 {
            g.settle_structures(crate::sim::STEP);
        }
        let fallen = &g.walls[wi];
        assert!(!fallen.falling && fallen.tree_topple.is_none());
        assert_eq!(fallen.cells.iter().filter(|&&v| v).count(), initial);
        let extent = Vec2::new(fallen.size[0] as f32, fallen.size[2] as f32) * CELL;
        let center = Vec2::new(fallen.origin.x, fallen.origin.z) + extent * 0.5;
        assert!(
            (center - Vec2::new(pivot.x, pivot.z)).dot(direction) > 0.5,
            "tree did not fall sideways: {:?}",
            fallen.size
        );
        let grid = Vec2::new(fallen.origin.x, fallen.origin.z) / CELL;
        assert!(grid.distance(grid.round()) < 0.0001);
        let revisions = fallen.revision;
        for _ in 0..60 {
            g.settle_structures(crate::sim::STEP);
        }
        assert_eq!(g.walls[wi].revision, revisions);
    }

    #[test]
    fn healthy_or_two_voxel_trunk_support_does_not_topple() {
        for supports in [2, 4] {
            let mut g = tree(42, supports, 3);
            for _ in 0..30 {
                g.settle_structures(crate::sim::STEP);
            }
            assert!(
                g.walls
                    .iter()
                    .all(|w| w.tree_topple.is_none() && !w.tree_felled && !w.falling)
            );
        }
    }

    #[test]
    fn sides_are_reproducible_varied_and_do_not_consume_combat_randomness() {
        let mut sides = BTreeSet::new();
        for seed in 1..=32 {
            let mut a = tree(seed, 1, 0);
            let mut b = a.clone();
            let rng = a.combat_rng.0;
            a.start_weakened_trees();
            b.start_weakened_trees();
            let at = a.walls.last().unwrap().tree_topple.as_ref().unwrap();
            let bt = b.walls.last().unwrap().tree_topple.as_ref().unwrap();
            assert_eq!(at.direction, bt.direction);
            assert_eq!(a.combat_rng.0, rng);
            sides.insert(at.direction);
        }
        assert_eq!(sides.len(), 4);
    }

    #[test]
    fn grid_poses_preserve_voxel_count_and_materials() {
        let mut g = tree(42, 0, 0);
        g.start_weakened_trees();
        let t = g.walls.last().unwrap().tree_topple.as_ref().unwrap();
        for step in 0..=16 {
            let pose = t.rasterize(step as f32 * POSE_STEP).unwrap();
            assert_eq!(pose.cells.iter().filter(|&&v| v).count(), t.voxels.len());
            for material in [1, 2] {
                assert_eq!(
                    pose.materials.iter().filter(|&&m| m == material).count(),
                    t.voxels.iter().filter(|v| v.material == material).count()
                );
            }
        }
    }

    #[test]
    fn falling_tree_breaks_around_player_without_restoring_voxels_or_repeated_damage() {
        let mut g = tree(42, 1, 0);
        for z in 128..149 {
            for x in 140..156 {
                g.terrain
                    .set_column(crate::terrain::Terrain::column_center(x, z), 0.0);
            }
        }
        g.start_weakened_trees();
        let wi = g.walls.len() - 1;
        g.walls[wi].tree_topple.as_mut().unwrap().direction = [1, 0];
        g.player = Vec2::new(5.0, 3.3);
        g.prev_player = g.player;
        g.player_body = crate::controller::Body::at(g.terrain.height(g.player));
        let initial = g.walls[wi].cells.iter().filter(|&&c| c).count();
        let hp = g.hp;
        for _ in 0..600 {
            // Let immunity expire: the same tree must still cause only one impact hit.
            g.invulnerability = (g.invulnerability - crate::sim::STEP).max(0.0);
            g.settle_structures(crate::sim::STEP);
        }
        assert!(g.walls[wi].tree_hit_player);
        assert!(g.hp < hp && g.hp >= hp - 10.001);
        assert!(g.walls[wi].cells.iter().filter(|&&c| c).count() < initial);
        assert!(crate::controller::clear(
            &g.terrain,
            &g.walls,
            g.player,
            g.player_body.y,
            &g.physics
        ));
        assert!(crate::controller::clear(
            &g.terrain,
            &g.walls,
            g.player + Vec2::new(-0.04, 0.0),
            g.player_body.y,
            &g.physics
        ));
        assert_eq!(g.events.iter().filter(|e| e["kind"] == "hurt").count(), 1);
        let start = g.player;
        crate::controller::advance(
            &g.terrain,
            &g.walls,
            &g.physics,
            &mut g.player,
            &mut g.player_body,
            Vec2::new(0.0, 1.5),
            0.1,
        );
        assert!(
            g.player.distance(start) > 1.0,
            "player remained trapped: {start:?} -> {:?}, tree {:?} {:?}",
            g.player,
            g.walls[wi].origin,
            g.walls[wi].size
        );
    }

    #[test]
    fn fast_downward_tree_settling_sweeps_player_and_breaks_even_during_immunity() {
        let mut g = tree(42, 4, 0);
        g.player = Vec2::new(5.0, 3.0);
        g.prev_player = g.player;
        g.player_body = crate::controller::Body::at(g.terrain.height(g.player));
        let mut w = Wall::new(Vec3::new(4.8, 4.0, 2.8), [2, 2, 2]);
        w.tree_felled = true;
        w.falling = true;
        w.material = 1;
        g.walls = vec![w];
        g.support_dirty.clear();
        g.support_revision = g.terrain.revision;
        let hp = g.hp;
        g.invulnerability = 0.5;
        g.settle_structures(1.0);
        assert!(g.walls[0].tree_hit_player);
        assert_eq!(g.hp, hp);
        assert!(g.walls[0].cells.iter().all(|&c| !c));
        assert!(crate::controller::clear(
            &g.terrain,
            &g.walls,
            g.player,
            g.player_body.y,
            &g.physics
        ));
    }

    #[test]
    fn damage_during_toppling_is_not_restored_by_the_next_pose() {
        let mut g = tree(42, 1, 0);
        g.settle_structures(crate::sim::STEP);
        let wi = g
            .walls
            .iter()
            .position(|w| w.tree_topple.is_some())
            .unwrap();
        let before = g.walls[wi].cells.iter().filter(|&&v| v).count();
        let i = g.walls[wi].cells.iter().position(|&v| v).unwrap();
        let p = g.walls[wi].center(i);
        g.chip_wall_at(wi, p, 0.11);
        assert_eq!(
            g.walls[wi]
                .tree_topple
                .as_ref()
                .unwrap()
                .voxels
                .iter()
                .filter(|v| v.material != 0)
                .count(),
            before - 1
        );
        for _ in 0..90 {
            g.settle_structures(crate::sim::STEP);
        }
        assert_eq!(g.walls[wi].cells.iter().filter(|&&v| v).count(), before - 1);
    }

    #[test]
    fn toppling_state_survives_checkpoint_and_stops_at_obstacles() {
        let mut g = tree(42, 1, 0);
        for _ in 0..50 {
            g.settle_structures(crate::sim::STEP);
        }
        let wi = g
            .walls
            .iter()
            .position(|w| w.tree_topple.is_some())
            .unwrap();
        g.phase = crate::sim::Phase::Shop;
        let path = std::env::temp_dir().join("hot-vampires-tree-topple-checkpoint.json");
        g.save(&path).unwrap();
        let mut loaded = Game::load(&path).unwrap();
        assert_eq!(
            loaded.walls[wi].tree_topple.as_ref().unwrap().pose_angle,
            g.walls[wi].tree_topple.as_ref().unwrap().pose_angle
        );
        assert_eq!(loaded.walls[wi].tree_sources, g.walls[wi].tree_sources);
        for _ in 0..600 {
            loaded.settle_structures(crate::sim::STEP);
        }
        assert!(!loaded.walls[wi].falling);
        std::fs::remove_file(path).unwrap();
        // A proposed grid pose must not be allowed to enter a solid neighboring wall.
        let t = g.walls[wi].tree_topple.as_ref().unwrap();
        let pose = t.rasterize(std::f32::consts::FRAC_PI_2).unwrap();
        g.walls.push(Wall::new(pose.origin, pose.size));
        assert!(g.tree_pose_blocked(wi, &pose));
    }

    #[test]
    fn destroying_soil_support_also_topples_the_whole_tree() {
        let mut g = tree(42, 4, 0);
        for z in 139..144 {
            for x in 139..144 {
                g.terrain
                    .set_column(crate::terrain::Terrain::column_center(x, z), -2.0);
            }
        }
        g.settle_structures(crate::sim::STEP);
        assert!(g.walls.iter().any(|w| w.tree_topple.is_some()));
    }

    #[test]
    fn fully_destroyed_toppling_tree_stops_simulating_empty_geometry() {
        let mut g = tree(42, 1, 0);
        g.settle_structures(crate::sim::STEP);
        let wi = g
            .walls
            .iter()
            .position(|w| w.tree_topple.is_some())
            .unwrap();
        let p = g.walls[wi].origin;
        g.chip_wall_at(wi, p, 100.0);
        for _ in 0..120 {
            g.settle_structures(crate::sim::STEP);
        }
        assert!(!g.walls[wi].falling && g.walls[wi].tree_topple.is_none());
    }

    #[test]
    #[ignore]
    fn profile_tree_toppling() {
        let mut g = Game::new(11, crate::content::Config::default());
        demo(&mut g);
        let mut cache = crate::render::SceneCache::default();
        cache.scene(&g, 0.4, 10.0, 1.6);
        g.navigation
            .update(&g.terrain, &g.walls, &g.physics, g.player, crate::sim::STEP);
        let mut frame = Vec::new();
        let mut physics = Vec::new();
        for _ in 0..180 {
            let start = std::time::Instant::now();
            let mut elapsed = 0.0;
            for _ in 0..2 {
                let t = std::time::Instant::now();
                g.settle_structures(crate::sim::STEP);
                elapsed += t.elapsed().as_secs_f64() * 1000.0;
                g.navigation
                    .update(&g.terrain, &g.walls, &g.physics, g.player, crate::sim::STEP);
            }
            std::hint::black_box(cache.scene(&g, 0.4, 10.0, 1.6));
            physics.push(elapsed);
            frame.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        fn stats(mut times: Vec<f64>) -> serde_json::Value {
            times.sort_by(f64::total_cmp);
            serde_json::json!({"mean_ms":times.iter().sum::<f64>()/times.len() as f64,"p95_ms":times[times.len()*95/100],"peak_ms":times.last()})
        }
        let report = serde_json::json!({"physics":stats(physics),"cpu_frame":stats(frame),"walls":g.walls.len(),"remaining_toppling":g.walls.iter().filter(|w|w.tree_topple.is_some()).count()});
        std::fs::write(
            "target/tree-topple-profile.json",
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
        println!("{report}");
        assert_eq!(
            g.walls.iter().filter(|w| w.tree_topple.is_some()).count(),
            0
        );
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Topple {
    pub pivot: Vec3,
    pub direction: [i32; 2],
    pub angle: f32,
    pub pose_angle: f32,
    pub speed: f32,
    pub voxels: Vec<TreeVoxel>,
}

struct Pose {
    origin: Vec3,
    size: [usize; 3],
    cells: Vec<bool>,
    materials: Vec<u8>,
    sources: Vec<u32>,
}

impl Topple {
    pub fn validate(&self) -> bool {
        self.pivot.is_finite()
            && matches!(self.direction, [1, 0] | [-1, 0] | [0, 1] | [0, -1])
            && [self.angle, self.pose_angle, self.speed]
                .iter()
                .all(|v| v.is_finite())
            && (0.0..=std::f32::consts::FRAC_PI_2).contains(&self.angle)
            && (0.0..=self.angle + 0.0001).contains(&self.pose_angle)
            && (0.0..=3.5).contains(&self.speed)
            && self.voxels.len() <= 100_000
            && self.voxels.iter().all(|v| {
                matches!(v.material, 0 | 1 | 2 | 7)
                    && v.cell.iter().all(|&c| (-100..=100).contains(&c))
            })
    }

    fn rasterize(&self, angle: f32) -> Option<Pose> {
        let (sin, cos) = angle.sin_cos();
        let d = Vec3::new(self.direction[0] as f32, 0.0, self.direction[1] as f32);
        // Iteration order never chooses a pose: keys are only queried, and each output
        // voxel is written by its grid index. Hash lookup avoids repeated tree searches.
        let mut occupied: std::collections::HashMap<[i32; 3], (u8, u32)> =
            std::collections::HashMap::with_capacity(self.voxels.len());
        for (source, voxel) in self.voxels.iter().enumerate() {
            if voxel.material == 0 {
                continue;
            }
            let p = IVec3::from_array(voxel.cell).as_vec3() + Vec3::splat(0.5);
            let along = p.dot(d);
            let rotated = p + d * (along * cos + p.y * sin - along);
            let ideal = Vec3::new(rotated.x, p.y * cos - along * sin, rotated.z) - Vec3::splat(0.5);
            let mut cell = ideal.round().as_ivec3().to_array();
            if matches!(voxel.material, 1 | 7) {
                cell[1] = cell[1].max(0);
            }
            // A forward rotation can map two source voxels to one cell. Preserve both
            // by choosing the closest free neighbor instead of shrinking the tree.
            if occupied.contains_key(&cell) {
                let center = IVec3::from_array(cell);
                let mut best = None;
                for radius in 1i32..=4 {
                    let mut distance = f32::INFINITY;
                    for z in -radius..=radius {
                        for y in -radius..=radius {
                            for x in -radius..=radius {
                                if x.abs().max(y.abs()).max(z.abs()) != radius {
                                    continue;
                                }
                                let candidate = center + IVec3::new(x, y, z);
                                if matches!(voxel.material, 1 | 7) && candidate.y < 0 {
                                    continue;
                                }
                                if occupied.contains_key(&candidate.to_array()) {
                                    continue;
                                }
                                let score = candidate.as_vec3().distance_squared(ideal);
                                if score < distance {
                                    distance = score;
                                    best = Some(candidate.to_array());
                                }
                            }
                        }
                    }
                    if best.is_some() {
                        break;
                    }
                }
                cell = best.expect("tree rasterization exhausted neighboring cells");
            }
            occupied.insert(cell, (voxel.material, source as u32));
        }
        let mut min = IVec3::splat(i32::MAX);
        let mut max = IVec3::splat(i32::MIN);
        for &cell in occupied.keys() {
            let p = IVec3::from_array(cell);
            min = min.min(p);
            max = max.max(p);
        }
        if occupied.is_empty() {
            return None;
        }
        let extent = max - min + IVec3::ONE;
        let size = [extent.x as usize, extent.y as usize, extent.z as usize];
        let n = size.iter().product();
        let mut pose = Pose {
            origin: self.pivot + min.as_vec3() * CELL,
            size,
            cells: vec![false; n],
            materials: vec![0; n],
            sources: vec![u32::MAX; n],
        };
        for (cell, (material, source)) in occupied {
            let p = IVec3::from_array(cell) - min;
            let i = p.x as usize + size[0] * (p.y as usize + size[1] * p.z as usize);
            pose.cells[i] = true;
            pose.materials[i] = material;
            pose.sources[i] = source;
        }
        Some(pose)
    }
}

impl Pose {
    fn apply(self, wall: &mut Wall) {
        wall.origin = self.origin;
        wall.size = self.size;
        wall.cells = self.cells;
        wall.voxel_materials = self.materials;
        wall.tree_sources = self.sources;
        wall.bottom_cells.clear();
        wall.contact_clearance = None;
        wall.revision += 1;
    }
}

impl Game {
    /// Bounded capsule query, with a little clearance so the player can leave the break.
    fn tree_impact_cells(
        &self,
        origin: Vec3,
        size: [usize; 3],
        cells: &[bool],
        drop: f32,
        maintain_escape: bool,
    ) -> Vec<usize> {
        let hit = self.tree_path_cells(origin, size, cells, drop, self.player);
        if hit.is_empty() && !maintain_escape {
            return hit;
        }
        // A capsule-sized cavity alone can imprison the player inside a wide canopy.
        // Break a short passage to the nearest side of this body as well.
        let exits = [
            Vec2::new(origin.x - self.physics.radius, self.player.y),
            Vec2::new(
                origin.x + size[0] as f32 * CELL + self.physics.radius,
                self.player.y,
            ),
            Vec2::new(self.player.x, origin.z - self.physics.radius),
            Vec2::new(
                self.player.x,
                origin.z + size[2] as f32 * CELL + self.physics.radius,
            ),
        ];
        let exit = *exits
            .iter()
            .min_by(|a, b| {
                a.distance_squared(self.player)
                    .total_cmp(&b.distance_squared(self.player))
            })
            .unwrap();
        self.tree_path_cells(origin, size, cells, drop, exit)
    }

    fn tree_path_cells(
        &self,
        origin: Vec3,
        size: [usize; 3],
        cells: &[bool],
        drop: f32,
        exit: Vec2,
    ) -> Vec<usize> {
        let radius = self.physics.radius + 0.06;
        let path_min = self.player.min(exit);
        let path_max = self.player.max(exit);
        let travel = exit - self.player;
        let feet = self.player_body.y;
        let low = feet + self.physics.radius;
        let high = feet + self.physics.body_height - self.physics.radius;
        let min = ((Vec3::new(path_min.x - radius, feet - drop - 0.06, path_min.y - radius)
            - origin)
            / CELL)
            .floor()
            .as_ivec3();
        let max = ((Vec3::new(
            path_max.x + radius,
            feet + self.physics.body_height + 0.06,
            path_max.y + radius,
        ) - origin)
            / CELL)
            .floor()
            .as_ivec3();
        let mut hit = Vec::new();
        for z in min.z.max(0)..=max.z.min(size[2] as i32 - 1) {
            for y in min.y.max(0)..=max.y.min(size[1] as i32 - 1) {
                for x in min.x.max(0)..=max.x.min(size[0] as i32 - 1) {
                    let i = x as usize + size[0] * (y as usize + size[1] * z as usize);
                    if !cells[i] {
                        continue;
                    }
                    let center =
                        origin + Vec3::new(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5) * CELL;
                    let q = Vec2::new(center.x, center.z);
                    let nearest = self.player
                        + travel
                            * ((q - self.player).dot(travel)
                                / travel.length_squared().max(0.000001))
                            .clamp(0.0, 1.0);
                    let horizontal =
                        ((nearest - q).abs() - Vec2::splat(CELL * 0.5)).max(Vec2::ZERO);
                    // Swept vertical box covers the entire downward move, even a long frame.
                    let bottom = center.y - CELL * 0.5;
                    let top = center.y + CELL * 0.5 + drop;
                    let vertical = (low - top).max(bottom - high).max(0.0);
                    if horizontal.length_squared() + vertical * vertical < radius * radius {
                        hit.push(i);
                    }
                }
            }
        }
        hit
    }

    fn tree_impact(&mut self, wi: usize, removed: usize) {
        let wall = &mut self.walls[wi];
        wall.modified = true;
        wall.revision += 1;
        wall.bottom_cells.clear();
        wall.contact_clearance = None;
        let first = !wall.tree_hit_player;
        wall.tree_hit_player = true;
        self.metrics.cover_removed += removed as u32;
        self.invalidate_support_near(wi);
        if first {
            self.hurt(10.0);
            self.emit(
                Vec3::new(self.player.x, self.player_body.y + 0.8, self.player.y),
                8,
                [0.38, 0.30, 0.16],
            );
        }
        self.event(
            "tree_impact",
            serde_json::json!({"wall":wi,"removed":removed,"first_hit":first}),
        );
    }

    fn break_tree_pose_on_player(
        &mut self,
        wi: usize,
        pose: &mut Pose,
        topple: &mut Topple,
    ) -> bool {
        let hit = self.tree_impact_cells(
            pose.origin,
            pose.size,
            &pose.cells,
            0.0,
            self.walls[wi].tree_hit_player,
        );
        if hit.is_empty() {
            return false;
        }
        for &i in &hit {
            pose.cells[i] = false;
            pose.materials[i] = 0;
            topple.voxels[pose.sources[i] as usize].material = 0;
        }
        // Intermediate sweep poses can hit before the next displayed pose. Remove those
        // source voxels from the current body too, including when an obstacle stops it.
        let wall = &mut self.walls[wi];
        for (i, &source) in wall.tree_sources.iter().enumerate() {
            if wall.cells[i]
                && topple
                    .voxels
                    .get(source as usize)
                    .is_some_and(|v| v.material == 0)
            {
                wall.cells[i] = false;
                wall.voxel_materials[i] = 0;
            }
        }
        self.tree_impact(wi, hit.len());
        true
    }

    pub(crate) fn break_settling_tree_on_player(&mut self, wi: usize, drop: f32) {
        let wall = &self.walls[wi];
        let hit = self.tree_impact_cells(
            wall.origin,
            wall.size,
            &wall.cells,
            drop,
            wall.tree_hit_player,
        );
        if hit.is_empty() {
            return;
        }
        let wall = &mut self.walls[wi];
        for &i in &hit {
            wall.cells[i] = false;
            if let Some(topple) = &mut wall.tree_topple
                && let Some(&source) = wall.tree_sources.get(i)
                && let Some(voxel) = topple.voxels.get_mut(source as usize)
            {
                voxel.material = 0;
            }
            if let Some(material) = wall.voxel_materials.get_mut(i) {
                *material = 0;
            }
        }
        self.tree_impact(wi, hit.len());
    }

    pub fn start_weakened_trees(&mut self) {
        let full = self.support_revision != self.terrain.revision;
        let mut triggers = Vec::new();
        for (wi, w) in self.walls.iter().enumerate() {
            let masonry = matches!(w.material, 0 | 7) && w.size[1] >= 12;
            if (w.material != 1 && !masonry)
                || w.tree_felled
                || w.falling
                || (!full && !self.support_dirty.contains(&wi))
            {
                continue;
            }
            if !self.support_dirty.contains(&wi)
                && w.support_terrain_stamp == Some(self.terrain_support_stamp(wi))
            {
                continue;
            }
            for y in 0..w.size[1].saturating_sub(1) {
                let mut support = 0;
                let mut above = false;
                for z in 0..w.size[2] {
                    for x in 0..w.size[0] {
                        support += usize::from(w.cells[x + w.size[0] * (y + w.size[1] * z)]);
                        above |= (y + 1..w.size[1])
                            .any(|ay| w.cells[x + w.size[0] * (ay + w.size[1] * z)]);
                    }
                }
                let weak = if masonry {
                    support <= (w.size[0] * w.size[2] / 4).max(1) && w.size[1] - y - 1 >= 8
                } else {
                    support <= 1
                };
                if weak && above {
                    triggers.push((wi, y as i32));
                    break;
                }
            }
            if !triggers.iter().any(|&(i, _)| i == wi) {
                let rooted = (0..w.size[2]).any(|z| {
                    (0..w.size[0]).any(|x| {
                        let i = x + w.size[0] * w.size[1] * z;
                        if !w.cells[i] {
                            return false;
                        }
                        let p = w.center(i);
                        [-0.49, 0.49].iter().any(|&dx| {
                            [-0.49, 0.49].iter().any(|&dz| {
                                self.terrain
                                    .height(Vec2::new(p.x + dx * CELL, p.z + dz * CELL))
                                    >= w.origin.y - 0.001
                            })
                        }) || self.walls.iter().enumerate().any(|(j, other)| {
                            j != wi
                                && !other.is_groundcover()
                                && !other.falling
                                && other.solid3(p - Vec3::Y * (CELL + 0.001))
                        })
                    })
                });
                if !rooted && w.cells.iter().any(|&v| v) {
                    triggers.push((wi, -1));
                }
            }
        }
        for (wi, cut) in triggers {
            self.begin_tree_topple(wi, cut);
        }
    }

    fn begin_tree_topple(&mut self, wi: usize, cut: i32) {
        let trunk = &self.walls[wi];
        let crowns: Vec<_> = self
            .walls
            .iter()
            .enumerate()
            .filter_map(|(i, w)| {
                (trunk.material == 1
                    && w.material == 2
                    && !w.tree_felled
                    && !w.falling
                    && w.tile == trunk.tile
                    && w.origin.x <= trunk.origin.x + trunk.size[0] as f32 * CELL
                    && w.origin.z <= trunk.origin.z + trunk.size[2] as f32 * CELL
                    && w.origin.x + w.size[0] as f32 * CELL >= trunk.origin.x
                    && w.origin.z + w.size[2] as f32 * CELL >= trunk.origin.z)
                    .then_some(i)
            })
            .collect();
        // The scenery hash chooses a side without altering combat RNG or replay order.
        let x = (trunk.origin.x / CELL).round() as i64;
        let z = (trunk.origin.z / CELL).round() as i64;
        let mut random = self.seed
            ^ (x as u64).wrapping_mul(0x9e3779b97f4a7c15)
            ^ (z as u64).wrapping_mul(0xbf58476d1ce4e5b9);
        random = (random ^ (random >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        random = (random ^ (random >> 27)).wrapping_mul(0x94d049bb133111eb);
        random ^= random >> 31;
        let direction = if trunk.material != 1 {
            let sign = if random & 1 == 0 { 1 } else { -1 };
            if trunk.size[0] >= trunk.size[2] {
                [0, sign]
            } else {
                [sign, 0]
            }
        } else {
            [[1, 0], [-1, 0], [0, 1], [0, -1]][(random & 3) as usize]
        };
        let pivot = Vec3::new(
            trunk.origin.x
                + if direction[0] > 0 {
                    trunk.size[0] as f32 * CELL
                } else {
                    0.0
                },
            trunk.origin.y + (cut + 1) as f32 * CELL,
            trunk.origin.z
                + if direction[1] > 0 {
                    trunk.size[2] as f32 * CELL
                } else {
                    0.0
                },
        );
        let tile = trunk.tile.clone();
        let mut voxels = BTreeMap::new();
        for i in std::iter::once(wi).chain(crowns.iter().copied()) {
            let w = &self.walls[i];
            let offset = ((w.origin - pivot) / CELL).round().as_ivec3();
            for (j, &solid) in w.cells.iter().enumerate() {
                if !solid || (i == wi && w.coord(j)[1] as i32 <= cut) {
                    continue;
                }
                let c = w.coord(j);
                if i != wi {
                    let local = ((w.origin
                        + Vec3::new(c[0] as f32, c[1] as f32, c[2] as f32) * CELL
                        - trunk.origin)
                        / CELL)
                        .round()
                        .as_ivec3();
                    if local.x >= 0
                        && local.z >= 0
                        && local.y >= 0
                        && local.x < trunk.size[0] as i32
                        && local.z < trunk.size[2] as i32
                        && local.y <= cut
                        && trunk.cells[local.x as usize
                            + trunk.size[0] * (local.y as usize + trunk.size[1] * local.z as usize)]
                    {
                        continue;
                    }
                }
                let cell = (offset + IVec3::new(c[0] as i32, c[1] as i32, c[2] as i32)).to_array();
                let material = if matches!(w.material, 0 | 7) {
                    7
                } else {
                    w.material
                };
                voxels.entry(cell).or_insert(material);
            }
        }
        let topple = Topple {
            pivot,
            direction,
            angle: 0.0,
            pose_angle: 0.0,
            speed: 0.0,
            voxels: voxels
                .into_iter()
                .map(|(cell, material)| TreeVoxel { cell, material })
                .collect(),
        };
        let Some(pose) = topple.rasterize(0.0) else {
            return;
        };
        for i in std::iter::once(wi).chain(crowns.iter().copied()) {
            let w = &mut self.walls[i];
            for j in 0..w.cells.len() {
                if i != wi || w.coord(j)[1] as i32 > cut {
                    w.cells[j] = false;
                }
            }
            w.tree_felled = true;
            w.modified = true;
            w.revision += 1;
            self.invalidate_support_near(i);
        }
        let mut tree = Wall::new(pose.origin, pose.size);
        tree.material = if topple.voxels.iter().all(|v| v.material == 7) {
            7
        } else {
            4
        };
        tree.tile = tile;
        tree.tree_felled = true;
        tree.modified = true;
        tree.falling = true;
        pose.apply(&mut tree);
        tree.tree_topple = Some(topple);
        self.walls.push(tree);
    }

    fn tree_pose_blocked(&self, wi: usize, pose: &Pose) -> bool {
        let hi = pose.origin
            + Vec3::new(
                pose.size[0] as f32,
                pose.size[1] as f32,
                pose.size[2] as f32,
            ) * CELL;
        let neighbors: Vec<_> = self
            .walls
            .iter()
            .enumerate()
            .filter_map(|(i, w)| {
                (i != wi
                    && w.origin.x < hi.x
                    && w.origin.z < hi.z
                    && !w.is_groundcover()
                    && w.origin.x + w.size[0] as f32 * CELL > pose.origin.x
                    && w.origin.z + w.size[2] as f32 * CELL > pose.origin.z)
                    .then_some(w)
            })
            .collect();
        for (i, &solid) in pose.cells.iter().enumerate() {
            if !solid {
                continue;
            }
            let x = i % pose.size[0];
            let y = (i / pose.size[0]) % pose.size[1];
            let z = i / (pose.size[0] * pose.size[1]);
            let center =
                pose.origin + Vec3::new(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5) * CELL;
            if y == 0 || !pose.cells[i - pose.size[0]] {
                for dx in [-0.49, 0.49] {
                    for dz in [-0.49, 0.49] {
                        if self
                            .terrain
                            .height(Vec2::new(center.x + dx * CELL, center.z + dz * CELL))
                            > center.y - CELL * 0.5 + 0.001
                        {
                            return true;
                        }
                    }
                }
            }
            if neighbors.iter().any(|w| {
                [-0.49, 0.0, 0.49]
                    .iter()
                    .any(|&dy| w.solid3(center + Vec3::Y * (CELL * dy)))
            }) {
                return true;
            }
        }
        false
    }

    pub fn step_toppling_trees(&mut self, dt: f32) {
        let mut changed = Vec::new();
        for wi in 0..self.walls.len() {
            let Some(mut topple) = self.walls[wi].tree_topple.take() else {
                continue;
            };
            let height = topple
                .voxels
                .iter()
                .map(|v| v.cell[1])
                .max()
                .unwrap_or(1)
                .max(1) as f32
                * CELL;
            topple.speed = (topple.speed + self.physics.gravity / height * dt).min(3.5);
            topple.angle = (topple.angle + topple.speed * dt).min(std::f32::consts::FRAC_PI_2);
            let target = (topple.angle / POSE_STEP).floor() * POSE_STEP;
            let mut finished = false;
            // Sweep intermediate grid poses so a fast tip cannot pass through thin walls.
            while topple.pose_angle + POSE_STEP <= target + 0.00001 {
                let next = (topple.pose_angle + POSE_STEP).min(std::f32::consts::FRAC_PI_2);
                let mut pose = None;
                for substep in 1..=4 {
                    let angle =
                        topple.pose_angle + (next - topple.pose_angle) * substep as f32 / 4.0;
                    let Some(mut candidate) = topple.rasterize(angle) else {
                        finished = true;
                        break;
                    };
                    if self.tree_pose_blocked(wi, &candidate) {
                        finished = true;
                        break;
                    }
                    if self.break_tree_pose_on_player(wi, &mut candidate, &mut topple) {
                        changed.push(wi);
                    }
                    if substep == 4 {
                        pose = Some(candidate);
                    }
                }
                if finished {
                    break;
                }
                if let Some(pose) = pose {
                    pose.apply(&mut self.walls[wi]);
                    changed.push(wi);
                }
                topple.pose_angle = next;
                if next >= std::f32::consts::FRAC_PI_2 - 0.00001 {
                    finished = true;
                    break;
                }
            }
            if finished {
                // The rotated body uses the existing cached gravity/contact solver to
                // close the final gap to the terrain, preserving solid collision.
                let w = &mut self.walls[wi];
                w.tree_sources.clear();
                w.bottom_cells.clear();
                w.contact_clearance = None;
                w.fall_speed = 0.0;
                w.fall_distance = 0.0;
                changed.push(wi);
            } else {
                self.walls[wi].tree_topple = Some(topple);
            }
            if self.walls[wi].tree_hit_player {
                self.break_settling_tree_on_player(wi, 0.0);
            }
        }
        for wi in changed {
            self.invalidate_support_near(wi);
        }
    }
}
