//! Elevation-aware reverse flow field. Terrain revisions invalidate it immediately.
use crate::{
    controller::{clear, support},
    sim::Wall,
    terrain::{COLS, Physics, Terrain},
};
use glam::Vec2;
#[derive(Clone, Debug, Default)]
pub struct Navigation {
    pub revision: u64,
    pub goal: Option<(usize, usize)>,
    pub distance: Vec<u32>,
    pub heights: Vec<f32>,
    pub clock: f32,
    pub walls: usize,
    pub rebuilds: u32,
    pub walk: Vec<bool>,
    pub stamps: Vec<u64>,
    pub step_height: f32,
    pub max_grade: f32,
}
impl Navigation {
    pub fn update(&mut self, t: &Terrain, walls: &[Wall], c: &Physics, player: Vec2, dt: f32) {
        self.step_height = c.step_height;
        self.max_grade = c.max_slope_degrees.to_radians().tan();
        self.clock -= dt;
        let goal = t.world_column(player);
        let count = walls.iter().enumerate().fold(0usize, |h, (i, w)| {
            if w.is_groundcover() {
                return h;
            }
            h.wrapping_add(
                (w.revision as usize).wrapping_mul(7919)
                    ^ (w.origin.y.to_bits() as usize)
                    ^ (w.origin.x.to_bits() as usize).rotate_left(7)
                    ^ (w.origin.z.to_bits() as usize).rotate_left(13)
                    ^ i.wrapping_mul(127)
                    ^ w.cells.len(),
            )
        });
        let moving_geometry = walls.iter().any(|w| w.falling && !w.is_groundcover());
        if self.distance.len() == COLS() * COLS()
            && self.revision == t.revision
            && (self.walls == count || (moving_geometry && self.clock > 0.0))
            && (self.goal == goal || self.clock > 0.0)
        {
            return;
        }
        // Soft vegetation never changes capsule support. Filter once per flow rebuild
        // rather than rechecking every plant for each of the 65,536 navigation cells.
        let blockers: Vec<_> = walls
            .iter()
            .filter(|w| !w.is_groundcover())
            .cloned()
            .collect();
        let walls = blockers.as_slice();
        self.goal = goal;
        self.revision = t.revision;
        self.walls = count;
        // Actual capsule collision is checked every tick; moving flow fields refresh at 10 Hz.
        self.clock = if moving_geometry { 0.1 } else { 0.2 };
        self.rebuilds += 1;
        self.heights.resize(COLS() * COLS(), 0.0);
        self.walk.resize(COLS() * COLS(), false);
        let first = self.stamps.is_empty();
        self.stamps
            .resize(crate::terrain::CHUNKS() * crate::terrain::CHUNKS(), 0);
        for cz in 0..crate::terrain::CHUNKS() {
            for cx in 0..crate::terrain::CHUNKS() {
                let index = cx + crate::terrain::CHUNKS() * cz;
                let lo = t.world_center(
                    cx * crate::terrain::CHUNK_CELLS,
                    cz * crate::terrain::CHUNK_CELLS,
                ) - Vec2::splat(c.radius + crate::terrain::XZ() * 0.5);
                let hi = lo
                    + Vec2::splat(
                        crate::terrain::CHUNK_CELLS as f32 * crate::terrain::XZ()
                            + c.radius * 2.0
                            + c.step_height,
                    );
                let stamp = walls
                    .iter()
                    .enumerate()
                    .filter(|(_, w)| {
                        !w.is_groundcover()
                            && w.origin.x <= hi.x
                            && w.origin.z <= hi.y
                            && w.origin.x + w.size[0] as f32 * crate::sim::CELL >= lo.x
                            && w.origin.z + w.size[2] as f32 * crate::sim::CELL >= lo.y
                    })
                    .fold(t.stamp(cx, cz), |h, (i, w)| {
                        h.rotate_left(7)
                            ^ w.revision.wrapping_mul(7919)
                            ^ w.origin.y.to_bits() as u64
                            ^ i as u64
                            ^ w.cells.len() as u64
                    });
                if !first && self.stamps[index] == stamp {
                    continue;
                }
                self.stamps[index] = stamp;
                for z in cz * crate::terrain::CHUNK_CELLS..(cz + 1) * crate::terrain::CHUNK_CELLS {
                    for x in
                        cx * crate::terrain::CHUNK_CELLS..(cx + 1) * crate::terrain::CHUNK_CELLS
                    {
                        let i = x + COLS() * z;
                        let p = t.world_center(x, z);
                        self.heights[i] = crate::controller::ground(
                            t,
                            walls,
                            p,
                            c.radius,
                            support(t, p, c.radius) + c.step_height,
                        );
                        self.walk[i] = clear(t, walls, p, self.heights[i], c);
                    }
                }
            }
        }
        self.distance = vec![u32::MAX; COLS() * COLS()];
        let Some((x, z)) = goal else {
            return;
        };
        let mut queue = std::collections::VecDeque::new();
        // Connect the actual pose to nearby centres with the shared swept controller.
        // At a rim the occupied cell's centre can lie on an entirely different level.
        for dz in -1..=1 {
            for dx in -1..=1 {
                let nx = x as i32 + dx;
                let nz = z as i32 + dz;
                if nx < 1 || nz < 1 || nx >= COLS() as i32 - 1 || nz >= COLS() as i32 - 1 {
                    continue;
                }
                let i = nx as usize + COLS() * nz as usize;
                if !self.walk[i] {
                    continue;
                }
                let end = t.world_center(nx as usize, nz as usize);
                let mut p = player;
                let mut body = crate::controller::Body::at(crate::controller::ground(
                    t,
                    walls,
                    player,
                    c.radius,
                    support(t, player, c.radius) + c.step_height,
                ));
                crate::controller::advance(t, walls, c, &mut p, &mut body, end - player, 0.0);
                if p.distance_squared(end) < 0.0004 && body.grounded {
                    self.distance[i] = 0;
                    queue.push_back(i);
                }
            }
        }
        while let Some(i) = queue.pop_front() {
            for (dx, dz) in [
                (-1, 0),
                (1, 0),
                (0, -1),
                (0, 1),
                (-1, -1),
                (-1, 1),
                (1, -1),
                (1, 1),
            ] {
                let x = (i % COLS()) as i32 + dx;
                let z = (i / COLS()) as i32 + dz;
                if x <= 0 || z <= 0 || x >= COLS() as i32 - 1 || z >= COLS() as i32 - 1 {
                    continue;
                }
                let n = x as usize + COLS() * z as usize;
                if !self.walk[n] || self.distance[n] != u32::MAX {
                    continue;
                }
                let dh = self.heights[i] - self.heights[n];
                let run = crate::terrain::XZ()
                    * if dx != 0 && dz != 0 {
                        std::f32::consts::SQRT_2
                    } else {
                        1.0
                    };
                if dh.abs() > c.step_height
                    || dh.abs() / run > c.max_slope_degrees.to_radians().tan()
                {
                    continue;
                }
                if dx != 0 && dz != 0 {
                    let a = x as usize + COLS() * (i / COLS());
                    let b = i % COLS() + COLS() * z as usize;
                    if !self.walk[a]
                        || !self.walk[b]
                        || (self.heights[a] - self.heights[n]).abs() > c.step_height
                        || (self.heights[b] - self.heights[n]).abs() > c.step_height
                    {
                        continue;
                    }
                }
                self.distance[n] = self.distance[i] + 1;
                queue.push_back(n);
            }
        }
    }
    pub fn direction(&self, t: &Terrain, p: Vec2, goal: Vec2) -> Option<Vec2> {
        let (x, z) = t.world_column(p)?;
        let i = x + COLS() * z;
        if self.distance.get(i).copied()? == u32::MAX {
            return None;
        }
        if self.distance[i] < 2 {
            return Some((goal - p).normalize_or_zero());
        }
        let mut best = i;
        for dx in -1..=1 {
            for dz in -1..=1 {
                let nx = x as i32 + dx;
                let nz = z as i32 + dz;
                if nx < 0 || nz < 0 || nx >= COLS() as i32 || nz >= COLS() as i32 {
                    continue;
                }
                let n = nx as usize + COLS() * nz as usize;
                let run = crate::terrain::XZ()
                    * if dx != 0 && dz != 0 {
                        std::f32::consts::SQRT_2
                    } else {
                        1.0
                    };
                let dh = (self.heights[n] - self.heights[i]).abs();
                let corner = dx != 0
                    && dz != 0
                    && (!self.walk[nx as usize + COLS() * z]
                        || !self.walk[x + COLS() * nz as usize]
                        || (self.heights[nx as usize + COLS() * z] - self.heights[i]).abs()
                            > self.step_height
                        || (self.heights[x + COLS() * nz as usize] - self.heights[i]).abs()
                            > self.step_height);
                if self.distance[n] < self.distance[best]
                    && dh <= self.step_height
                    && dh / run <= self.max_grade
                    && !corner
                {
                    best = n;
                }
            }
        }
        if best == i {
            None
        } else {
            Some((t.world_center(best % COLS(), best / COLS()) - p).normalize_or_zero())
        }
    }
}
