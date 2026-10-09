//! Deterministic streamed scenery and grid-stepped structural gravity.
use crate::sim::{CELL, Game, Wall};
use glam::{Vec2, Vec3};
use std::collections::{BTreeMap, BTreeSet};

fn hash(seed: u64, x: i32, z: i32) -> u64 {
    let mut h = seed
        ^ (x as u64).wrapping_mul(0x9e3779b97f4a7c15)
        ^ (z as u64).wrapping_mul(0xbf58476d1ce4e5b9);
    h = (h ^ (h >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94d049bb133111eb);
    h ^ (h >> 31)
}

/// Reproducible demonstration of a weakened tall ruin collapsing sideways.
pub fn ruin_demo(g: &mut Game) {
    let wi = g
        .walls
        .iter()
        .enumerate()
        .filter(|(_, w)| w.material == 7)
        .min_by(|(_, a), (_, b)| {
            a.origin
                .length_squared()
                .total_cmp(&b.origin.length_squared())
        })
        .map(|(i, _)| i)
        .unwrap();
    let wall = &mut g.walls[wi];
    for z in 0..wall.size[2] {
        for x in 0..wall.size[0] {
            if x != 0 || z != 0 {
                wall.cells[x + wall.size[0] * (2 + wall.size[1] * z)] = false;
            }
        }
    }
    let origin = wall.origin;
    wall.modified = true;
    wall.revision += 1;
    g.invalidate_support_near(wi);
    g.player = Vec2::new(origin.x - 1.5, origin.z + 1.0);
    g.prev_player = g.player;
    g.player_body = crate::controller::Body::at(g.terrain.height(g.player));
    g.config.min_scale = 1.0;
    g.scale = 1.0;
    g.config.wave_seconds = 600.0;
    g.spawn = 1_000_000.0;
    g.build.clear();
    g.enemies.clear();
}

impl Game {
    pub(crate) fn terrain_support_stamp(&self, wi: usize) -> u64 {
        let w = &self.walls[wi];
        let lo = ((Vec2::new(w.origin.x, w.origin.z) - self.terrain.origin
            + Vec2::splat(crate::terrain::HALF - CELL))
            / 8.0)
            .floor()
            .as_ivec2();
        let hi = ((Vec2::new(
            w.origin.x + w.size[0] as f32 * CELL,
            w.origin.z + w.size[2] as f32 * CELL,
        ) - self.terrain.origin
            + Vec2::splat(crate::terrain::HALF + CELL))
            / 8.0)
            .floor()
            .as_ivec2();
        let mut stamp = self.seed;
        for z in lo.y.max(0)..=hi.y.min(crate::terrain::CHUNKS() as i32 - 1) {
            for x in lo.x.max(0)..=hi.x.min(crate::terrain::CHUNKS() as i32 - 1) {
                stamp = stamp.rotate_left(11) ^ self.terrain.stamp(x as usize, z as usize);
            }
        }
        stamp
    }
    fn generate_ruins(&mut self, tx: i32, tz: i32, tile: &str) {
        let h = hash(self.seed ^ 0x726f696e7357616c, tx, tz);
        if (tx, tz) != (0, -1) && h % 3 != 0 {
            return;
        }
        let p = Vec2::new(tx as f32 * 16.0 + 6.0, tz as f32 * 16.0 + 5.0);
        if p.length_squared() < 64.0 {
            return;
        }
        let tall = 22 + ((h >> 12) & 7) as usize;
        for (offset, size) in [
            (Vec2::ZERO, [16, tall, 2]),
            (Vec2::new(3.6, 0.0), [2, tall - 4, 12]),
        ] {
            let p = p + offset;
            let mut foundation = f32::INFINITY;
            for z in 0..size[2] {
                for x in 0..size[0] {
                    foundation = foundation.min(
                        self.terrain
                            .height(p + Vec2::new(x as f32 + 0.5, z as f32 + 0.5) * CELL),
                    );
                }
            }
            let mut wall = Wall::new(Vec3::new(p.x, foundation, p.y), size);
            wall.material = 7;
            wall.tile = Some(tile.to_owned());
            for i in 0..wall.cells.len() {
                let [x, y, z] = wall.coord(i);
                let along = if size[0] > size[2] { x } else { z };
                let top = size[1] - (along / 3 % 3) * 2;
                let window = (5..=9).contains(&along) && (5..=11).contains(&y);
                wall.cells[i] = y < top && !window;
            }
            self.walls.push(wall);
        }
    }
    fn generate_groundcover(&mut self, tx: i32, tz: i32, tile: &str) {
        // Independent seed stream: scenery never consumes combat randomness.
        for slot in 0..14 {
            let h = hash(
                self.seed ^ (slot as u64 + 1).wrapping_mul(0x517cc1b727220a95),
                tx,
                tz,
            );
            let p = (Vec2::new(
                tx as f32 * 16.0 + 0.8 + (h & 255) as f32 / 18.0,
                tz as f32 * 16.0 + 0.8 + ((h >> 8) & 255) as f32 / 18.0,
            ) / CELL)
                .floor()
                * CELL;
            if p.length_squared() < 9.0 {
                continue;
            }
            let bush = slot >= 12;
            let size = if bush { [5, 5, 5] } else { [3, 3, 3] };
            let center = p + Vec2::splat(size[0] as f32 * CELL * 0.5);
            let mut plant = Wall::new(Vec3::new(p.x, self.terrain.height(center), p.y), size);
            plant.material = if bush { 6 } else { 5 };
            plant.tile = Some(tile.to_owned());
            for i in 0..plant.cells.len() {
                let [x, y, z] = plant.coord(i);
                plant.cells[i] = if bush {
                    let q = Vec3::new(
                        (x as f32 + 0.5 - 2.5) / 2.5,
                        y as f32 / 4.5,
                        (z as f32 + 0.5 - 2.5) / 2.5,
                    );
                    q.length_squared() < 1.0
                } else {
                    // Three staggered voxel blades, with a different tallest blade per tuft.
                    [(0, 1), (1, 2), (2, 0)].contains(&(x, z))
                        && y <= ((h >> (18 + x * 2)) & 3).min(2) as usize
                };
            }
            self.walls.push(plant);
        }
    }
    pub fn invalidate_support_near(&mut self, wi: usize) {
        let wall = &self.walls[wi];
        let lo = Vec2::new(wall.origin.x, wall.origin.z) - Vec2::splat(CELL);
        let hi = Vec2::new(
            wall.origin.x + wall.size[0] as f32 * CELL,
            wall.origin.z + wall.size[2] as f32 * CELL,
        ) + Vec2::splat(CELL);
        for (i, w) in self.walls.iter().enumerate() {
            if w.origin.x <= hi.x
                && (!wall.is_groundcover() || w.is_groundcover())
                && w.origin.z <= hi.y
                && w.origin.x + w.size[0] as f32 * CELL >= lo.x
                && w.origin.z + w.size[2] as f32 * CELL >= lo.y
            {
                self.support_dirty.insert(i);
            }
        }
    }
    pub fn stream_props(&mut self) {
        let center = (self.terrain.origin / 16.0).floor().as_ivec2();
        let mut desired = BTreeSet::new();
        for z in center.y - 2..=center.y + 2 {
            for x in center.x - 2..=center.x + 2 {
                desired.insert(format!("{x},{z}"));
            }
        }
        let mut leaving: BTreeMap<String, Vec<Wall>> = BTreeMap::new();
        self.walls.retain(|w| {
            if let Some(tile) = &w.tile
                && !desired.contains(tile)
            {
                leaving.entry(tile.clone()).or_default().push(w.clone());
                false
            } else {
                true
            }
        });
        for (key, walls) in leaving {
            if walls.iter().any(|w| w.modified) {
                self.archived_props.insert(key, walls);
            }
        }
        let active: BTreeSet<_> = self.walls.iter().filter_map(|w| w.tile.clone()).collect();
        for z in center.y - 2..=center.y + 2 {
            for x in center.x - 2..=center.x + 2 {
                let key = format!("{x},{z}");
                if active.contains(&key) {
                    continue;
                }
                if let Some(walls) = self.archived_props.remove(&key) {
                    self.walls.extend(walls);
                    continue;
                }
                let h = hash(self.seed, x, z);
                self.generate_groundcover(x, z, &key);
                self.generate_ruins(x, z, &key);
                let p = Vec2::new(
                    x as f32 * 16.0 + 3.0 + (h & 127) as f32 / 16.0,
                    z as f32 * 16.0 + 3.0 + ((h >> 8) & 127) as f32 / 16.0,
                );
                if p.length() < 12.0 {
                    continue;
                }
                let p = (p / CELL).floor() * CELL;
                let y = self.terrain.height(p);
                let tall = 14 + ((h >> 16) & 7) as usize;
                let mut trunk = Wall::new(Vec3::new(p.x, y, p.y), [2, tall, 2]);
                trunk.material = 1;
                trunk.tile = Some(key.clone());
                self.walls.push(trunk);
                let mut crown = Wall::new(
                    Vec3::new(
                        p.x - CELL * 4.0,
                        y + CELL * (tall - 2) as f32,
                        p.y - CELL * 4.0,
                    ),
                    [10, 10, 10],
                );
                crown.material = 2;
                crown.tile = Some(key.clone());
                for i in 0..crown.cells.len() {
                    let c = crown.coord(i);
                    let d = Vec3::new(
                        c[0] as f32 + 0.5,
                        (c[1] as f32 + 0.5) * 0.9,
                        c[2] as f32 + 0.5,
                    ) - Vec3::new(5.0, 4.5, 5.0);
                    crown.cells[i] = d.length_squared() < 25.0;
                }
                self.walls.push(crown);
                let rp = (p + Vec2::new(5.0, -4.0)) / CELL;
                let rp = rp.floor() * CELL;
                let mut rock = Wall::new(Vec3::new(rp.x, self.terrain.height(rp), rp.y), [8, 6, 8]);
                rock.material = 3;
                rock.tile = Some(key);
                for i in 0..rock.cells.len() {
                    let c = rock.coord(i);
                    let d = Vec3::new(
                        (c[0] as f32 + 0.5 - 4.0) / 4.0,
                        c[1] as f32 / 5.0,
                        (c[2] as f32 + 0.5 - 4.0) / 4.0,
                    );
                    rock.cells[i] = d.length_squared() < 1.0;
                }
                self.walls.push(rock);
            }
        }
        self.support_revision = u64::MAX;
    }

    /// Split connected pieces; support must come from actual soil or surviving cells.
    fn detach_unsupported(&mut self) {
        let full = self.support_revision != self.terrain.revision;
        let dirty = std::mem::take(&mut self.support_dirty);
        let mut fragments = Vec::new();
        for wi in 0..self.walls.len() {
            if self.walls[wi].falling || (!full && !dirty.contains(&wi)) {
                continue;
            }
            let stamp = self.terrain_support_stamp(wi);
            if !dirty.contains(&wi) && self.walls[wi].support_terrain_stamp == Some(stamp) {
                continue;
            }
            self.walls[wi].support_terrain_stamp = Some(stamp);
            let wall = &self.walls[wi];
            let mut visited = vec![false; wall.cells.len()];
            let mut detached = Vec::new();
            for start in 0..wall.cells.len() {
                if !wall.cells[start] || visited[start] {
                    continue;
                }
                visited[start] = true;
                let mut stack = vec![start];
                let mut component = Vec::new();
                let mut supported = false;
                while let Some(i) = stack.pop() {
                    component.push(i);
                    let c = wall.coord(i);
                    let q = wall.center(i);
                    if !supported && (c[1] == 0 || !wall.cells[i - wall.size[0]]) {
                        let bottom = q.y - CELL * 0.5;
                        supported |= self.terrain.height(Vec2::new(q.x, q.z)) >= bottom - 0.001;
                        for dx in [-0.49, 0.49] {
                            for dz in [-0.49, 0.49] {
                                supported |= self
                                    .terrain
                                    .height(Vec2::new(q.x + CELL * dx, q.z + CELL * dz))
                                    >= bottom - 0.001;
                            }
                        }
                        if !supported {
                            supported = self.walls.iter().enumerate().any(|(other, w)| {
                                other != wi
                                    && (!w.is_groundcover() || wall.is_groundcover())
                                    && !w.falling
                                    && w.solid3(q - Vec3::Y * (CELL + 0.001))
                            });
                        }
                    }
                    for axis in 0..3 {
                        for d in [-1isize, 1] {
                            let t = c[axis] as isize + d;
                            if t < 0 || t >= wall.size[axis] as isize {
                                continue;
                            }
                            let mut n = c;
                            n[axis] = t as usize;
                            let j = n[0] + wall.size[0] * (n[1] + wall.size[1] * n[2]);
                            if wall.cells[j] && !visited[j] {
                                visited[j] = true;
                                stack.push(j);
                            }
                        }
                    }
                }
                if !supported {
                    let mut min = [usize::MAX; 3];
                    let mut max = [0; 3];
                    for &i in &component {
                        let c = wall.coord(i);
                        for a in 0..3 {
                            min[a] = min[a].min(c[a]);
                            max[a] = max[a].max(c[a]);
                        }
                    }
                    let mut piece = Wall::new(
                        wall.origin + Vec3::new(min[0] as f32, min[1] as f32, min[2] as f32) * CELL,
                        [
                            max[0] - min[0] + 1,
                            max[1] - min[1] + 1,
                            max[2] - min[2] + 1,
                        ],
                    );
                    piece.cells.fill(false);
                    piece.material = wall.material;
                    piece.tree_felled = wall.tree_felled;
                    piece.tree_hit_player = wall.tree_hit_player;
                    if !wall.voxel_materials.is_empty() {
                        piece.voxel_materials = vec![0; piece.cells.len()];
                    }
                    piece.tile = wall.tile.clone();
                    piece.modified = true;
                    piece.falling = true;
                    for &i in &component {
                        let c = wall.coord(i);
                        let j = c[0] - min[0]
                            + piece.size[0] * (c[1] - min[1] + piece.size[1] * (c[2] - min[2]));
                        piece.cells[j] = true;
                        if !piece.voxel_materials.is_empty() {
                            piece.voxel_materials[j] = wall.voxel_materials[i];
                        }
                    }
                    detached.extend(component);
                    fragments.push(piece);
                }
            }
            if !detached.is_empty() {
                let wall = &mut self.walls[wi];
                for i in detached {
                    wall.cells[i] = false;
                }
                wall.modified = true;
                wall.revision += 1;
                self.invalidate_support_near(wi);
            }
        }
        self.walls.extend(fragments);
    }

    pub fn settle_structures(&mut self, dt: f32) {
        if self.support_revision != self.terrain.revision || !self.support_dirty.is_empty() {
            self.start_weakened_trees();
            self.detach_unsupported();
            self.support_revision = self.terrain.revision;
        }
        self.step_toppling_trees(dt);
        let mut changed = Vec::new();
        for wi in 0..self.walls.len() {
            if !self.walls[wi].falling || self.walls[wi].tree_topple.is_some() {
                continue;
            }
            if !self.walls[wi].cells.iter().any(|&v| v) {
                let wall = &mut self.walls[wi];
                wall.falling = false;
                wall.fall_speed = 0.0;
                wall.fall_distance = 0.0;
                wall.revision += 1;
                changed.push(wi);
                continue;
            }
            if self.walls[wi].bottom_cells.is_empty() {
                let wall = &mut self.walls[wi];
                wall.bottom_cells = wall
                    .cells
                    .iter()
                    .enumerate()
                    .filter_map(|(i, &solid)| {
                        (solid && (wall.coord(i)[1] == 0 || !wall.cells[i - wall.size[0]]))
                            .then_some(i)
                    })
                    .collect();
            }
            let wall = &self.walls[wi];
            let neighbors: Vec<_> = self
                .walls
                .iter()
                .enumerate()
                .filter_map(|(other, w)| {
                    (other != wi
                        && (!w.is_groundcover() || wall.is_groundcover())
                        && w.origin.x < wall.origin.x + wall.size[0] as f32 * CELL
                        && w.origin.z < wall.origin.z + wall.size[2] as f32 * CELL
                        && w.origin.x + w.size[0] as f32 * CELL > wall.origin.x
                        && w.origin.z + w.size[2] as f32 * CELL > wall.origin.z)
                        .then_some(other)
                })
                .collect();
            let stamp = neighbors.iter().fold(self.terrain.revision, |h, &i| {
                h.rotate_left(7)
                    ^ self.walls[i].revision.wrapping_mul(7919)
                    ^ self.walls[i].origin.y.to_bits() as u64
                    ^ i as u64
            });
            let cached = wall
                .contact_clearance
                .filter(|_| wall.contact_stamp == stamp);
            let mut clearance = cached.unwrap_or(f32::INFINITY);
            if cached.is_none() {
                for &i in &wall.bottom_cells {
                    let q = wall.center(i);
                    let bottom = q.y - CELL * 0.5;
                    let mut floor = self.terrain.height(Vec2::new(q.x, q.z));
                    for dx in [-0.49, 0.49] {
                        for dz in [-0.49, 0.49] {
                            floor = floor.max(
                                self.terrain
                                    .height(Vec2::new(q.x + CELL * dx, q.z + CELL * dz)),
                            );
                        }
                    }
                    for &other in &neighbors {
                        let w = &self.walls[other];
                        let local = ((q - w.origin) / CELL).floor().as_ivec3();
                        if local.x < 0
                            || local.z < 0
                            || local.x >= w.size[0] as i32
                            || local.z >= w.size[2] as i32
                        {
                            continue;
                        }
                        for y in (0..w.size[1]).rev() {
                            let top = w.origin.y + (y + 1) as f32 * CELL;
                            if top <= bottom + 0.001
                                && w.cells[local.x as usize
                                    + w.size[0] * (y + w.size[1] * local.z as usize)]
                            {
                                floor = floor.max(top);
                                break;
                            }
                        }
                    }
                    clearance = clearance.min((bottom - floor).max(0.0));
                }
            }
            let wall = &mut self.walls[wi];
            let old_y = wall.origin.y;
            wall.contact_stamp = stamp;
            wall.contact_clearance = Some(clearance);
            wall.fall_speed += self.physics.gravity * dt;
            wall.fall_distance += wall.fall_speed * dt;
            // Motion occupies whole grid cells; landing clamps exactly to the voxel surface.
            let drop = (wall.fall_distance / CELL).floor() * CELL;
            if wall.fall_distance >= clearance {
                wall.origin.y -= clearance;
                wall.falling = false;
                wall.fall_speed = 0.0;
                wall.fall_distance = 0.0;
                wall.revision += 1;
                wall.contact_clearance = Some(0.0);
                changed.push(wi);
            } else if drop > 0.0 {
                wall.origin.y -= drop;
                wall.fall_distance -= drop;
                wall.revision += 1;
                wall.contact_clearance = Some((clearance - drop).max(0.0));
                changed.push(wi);
            }
            let drop = old_y - self.walls[wi].origin.y;
            if drop > 0.0 && self.walls[wi].tree_felled {
                self.break_settling_tree_on_player(wi, drop);
            }
        }
        for wi in changed {
            self.invalidate_support_near(wi);
        }
    }
}
