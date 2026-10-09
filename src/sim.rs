use crate::{
    content::{Card, Config},
    shop::{Owned, Shop},
};
use glam::{Vec2, Vec3};
use serde::{Deserialize, Serialize};
pub const STEP: f32 = 1.0 / 120.0;
pub const CELL: f32 = 0.2;
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Phase {
    Combat,
    Shop,
    Victory,
    Dead,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Rng(pub u64);
impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    pub fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u32 << 24) as f32
    }
    pub fn index(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Enemy {
    #[serde(default)]
    pub body: crate::controller::Body,
    pub id: u64,
    pub pos: Vec2,
    pub prev: Vec2,
    pub hp: f32,
    pub kind: u32,
    pub clock: f32,
    pub direction: Vec2,
    pub chipped: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Shot {
    #[serde(default)]
    pub y: f32,
    #[serde(default)]
    pub prev_y: f32,
    #[serde(default)]
    pub velocity_y: f32,
    pub pos: Vec2,
    pub prev: Vec2,
    pub velocity: Vec2,
    pub life: f32,
    pub damage: f32,
    pub hostile: bool,
    pub pierce: u32,
    pub hit: Vec<u64>,
    pub primary: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Grenade {
    pub pos: Vec3,
    pub prev: Vec3,
    pub velocity: Vec3,
    pub fuse: f32,
    pub damage: f32,
    pub radius: f32,
    pub strength: f32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Debris {
    pub pos: Vec3,
    pub prev: Vec3,
    pub velocity: Vec3,
    pub life: f32,
    pub color: [f32; 3],
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Wall {
    pub origin: Vec3,
    pub size: [usize; 3],
    pub cells: Vec<bool>,
}
impl Wall {
    pub fn new(origin: Vec3, size: [usize; 3]) -> Self {
        Self {
            origin,
            size,
            cells: vec![true; size.iter().product()],
        }
    }
    pub fn coord(&self, i: usize) -> [usize; 3] {
        [
            i % self.size[0],
            (i / self.size[0]) % self.size[1],
            i / (self.size[0] * self.size[1]),
        ]
    }
    pub fn center(&self, i: usize) -> Vec3 {
        let [x, y, z] = self.coord(i);
        self.origin + Vec3::new(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5) * CELL
    }
    pub fn solid(&self, p: Vec2, r: f32) -> bool {
        let x0 = ((p.x - r - self.origin.x) / CELL).floor() as isize;
        let x1 = ((p.x + r - self.origin.x) / CELL).floor() as isize;
        let z0 = ((p.y - r - self.origin.z) / CELL).floor() as isize;
        let z1 = ((p.y + r - self.origin.z) / CELL).floor() as isize;
        if x1 < 0 || z1 < 0 || x0 >= self.size[0] as isize || z0 >= self.size[2] as isize {
            return false;
        }
        for z in z0.max(0)..=z1.min(self.size[2] as isize - 1) {
            for x in x0.max(0)..=x1.min(self.size[0] as isize - 1) {
                for y in 0..self.size[1].min(6) {
                    if self.cells[x as usize + self.size[0] * (y + self.size[1] * z as usize)] {
                        return true;
                    }
                }
            }
        }
        false
    }
    pub fn solid3(&self, p: Vec3) -> bool {
        let c = ((p - self.origin) / CELL).floor().as_ivec3();
        if c.min_element() < 0
            || c.x >= self.size[0] as i32
            || c.y >= self.size[1] as i32
            || c.z >= self.size[2] as i32
        {
            return false;
        }
        self.cells[c.x as usize + self.size[0] * (c.y as usize + self.size[1] * c.z as usize)]
    }
    pub fn support(&mut self) -> Vec<Vec3> {
        let mut reached = vec![false; self.cells.len()];
        let mut stack = Vec::new();
        for (i, &v) in self.cells.iter().enumerate() {
            if v && self.coord(i)[1] == 0 {
                reached[i] = true;
                stack.push(i);
            }
        }
        while let Some(i) = stack.pop() {
            let c = self.coord(i);
            for axis in 0..3 {
                for d in [-1isize, 1] {
                    let t = c[axis] as isize + d;
                    if t < 0 || t >= self.size[axis] as isize {
                        continue;
                    }
                    let mut n = c;
                    n[axis] = t as usize;
                    let j = n[0] + self.size[0] * (n[1] + self.size[1] * n[2]);
                    if self.cells[j] && !reached[j] {
                        reached[j] = true;
                        stack.push(j);
                    }
                }
            }
        }
        let mut removed = Vec::new();
        for (i, &r) in reached.iter().enumerate() {
            if self.cells[i] && !r {
                removed.push(self.center(i));
                self.cells[i] = false;
            }
        }
        removed
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Metrics {
    pub terrain_edits: u32,
    pub removed_voxels: u64,
    pub dirty_chunks: u64,
    pub falls: u32,
    pub no_progress_seconds: f32,
    pub stuck_reports: u32,
    pub unreachable_reports: u32,
    pub grounding_errors: u32,
    pub penetration_errors: u32,
    pub vertical_spikes: u32,
    pub navigation_rebuilds: u32,
    pub trapped_reports: u32,
    #[serde(skip)]
    pub terrain_update_us: u64,
    #[serde(skip)]
    pub navigation_update_us: u64,
    pub kills: u32,
    pub earned: u32,
    pub spent: u32,
    pub purchases: u32,
    pub rerolls: u32,
    pub heals: u32,
    pub dealt: f32,
    pub received: f32,
    pub cover_removed: u32,
    pub debris_peak: usize,
    pub standing: f32,
    pub overloads: u32,
    pub snapshots: Vec<serde_json::Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Game {
    #[serde(default)]
    pub terrain: crate::terrain::Terrain,
    #[serde(default)]
    pub physics: crate::terrain::Physics,
    #[serde(default)]
    pub player_body: crate::controller::Body,
    #[serde(default)]
    pub grenades: Vec<Grenade>,
    #[serde(default)]
    pub grenade_timer: f32,
    #[serde(skip)]
    pub navigation: crate::navigation::Navigation,
    pub schema: u32,
    pub seed: u64,
    pub config: Config,
    #[serde(skip)]
    pub cards: Vec<Card>,
    pub phase: Phase,
    pub wave: u32,
    pub player: Vec2,
    pub prev_player: Vec2,
    pub facing: f32,
    pub hp: f32,
    pub currency: u32,
    pub build: Vec<Owned>,
    pub shop: Shop,
    pub combat_rng: Rng,
    pub shop_rng: Rng,
    pub visual_rng: Rng,
    pub scale: f32,
    pub world: f32,
    pub real: f32,
    pub wave_time: f32,
    pub accumulator: f32,
    pub spawn: f32,
    pub cooldown: [f32; 3],
    pub activations: [u32; 3],
    pub invulnerability: f32,
    pub enemies: Vec<Enemy>,
    pub shots: Vec<Shot>,
    pub walls: Vec<Wall>,
    pub loot: Vec<Vec2>,
    pub debris: Vec<Debris>,
    pub next_id: u64,
    #[serde(default)]
    pub elite_spawned: bool,
    pub stationary: f32,
    pub moving: f32,
    pub inertia: bool,
    pub momentum: f32,
    pub metrics: Metrics,
    #[serde(skip)]
    pub events: Vec<serde_json::Value>,
}
impl Game {
    pub fn new(seed: u64, config: Config) -> Self {
        config.validate().unwrap();
        let seed = seed.max(1);
        Self {
            schema: 2,
            terrain: crate::terrain::Terrain::new(seed),
            physics: crate::terrain::Physics::default(),
            player_body: crate::controller::Body::at(0.0),
            grenades: Vec::new(),
            grenade_timer: 0.0,
            navigation: crate::navigation::Navigation::default(),
            seed,
            config,
            cards: crate::content::load(),
            phase: Phase::Combat,
            wave: 1,
            player: Vec2::ZERO,
            prev_player: Vec2::ZERO,
            facing: 0.0,
            hp: 65.0,
            currency: 0,
            build: vec![
                Owned {
                    id: "grenade".into(),
                    level: 1,
                    spent: 0,
                },
                Owned {
                    id: "needle".into(),
                    level: 1,
                    spent: 0,
                },
            ],
            shop: Shop::default(),
            combat_rng: Rng(seed),
            shop_rng: Rng(seed ^ 0x1234abcf),
            visual_rng: Rng(seed ^ 0x43219876),
            scale: 0.03,
            world: 0.0,
            real: 0.0,
            wave_time: 0.0,
            accumulator: 0.0,
            spawn: 0.0,
            cooldown: [0.0; 3],
            activations: [0; 3],
            invulnerability: 0.0,
            enemies: Vec::new(),
            shots: Vec::new(),
            walls: vec![
                Wall::new(Vec3::new(-3.0, 0.0, 3.0), [30, 8, 2]),
                Wall::new(Vec3::new(5.0, 0.0, -4.0), [2, 10, 25]),
                Wall::new(Vec3::new(-7.0, 0.0, -5.0), [15, 6, 2]),
            ],
            loot: Vec::new(),
            debris: Vec::new(),
            next_id: 1,
            elite_spawned: false,
            stationary: 0.0,
            moving: 0.0,
            inertia: false,
            momentum: 0.0,
            metrics: Metrics::default(),
            events: Vec::new(),
        }
    }
    pub fn event(&mut self, kind: &str, data: serde_json::Value) {
        self.events.push(serde_json::json!({"world":self.world,"real":self.real,"wave":self.wave,"kind":kind,"data":data}));
    }
    pub fn frame(&mut self, real_dt: f32, input: Vec2) {
        if self.phase != Phase::Combat {
            return;
        }
        let dt = real_dt.clamp(0.0, 0.1);
        self.real += dt;
        let input = input.clamp_length_max(1.0);
        let activity = input.length();
        let target = self.config.min_scale + (1.0 - self.config.min_scale) * activity;
        self.scale += (target - self.scale) * (1.0 - (-dt / self.config.transition_seconds).exp());
        self.scale = self.scale.clamp(self.config.min_scale, 1.0);
        if activity < 0.01 {
            self.metrics.standing += dt;
        }
        self.accumulator += dt * self.scale;
        let mut steps = 0;
        while self.accumulator >= STEP && steps < 16 && self.phase == Phase::Combat {
            self.tick(input);
            self.accumulator = (self.accumulator - STEP).max(0.0);
            steps += 1;
        }
        if self.accumulator >= STEP {
            self.accumulator = 0.0;
            self.metrics.overloads += 1;
            self.event("overload", serde_json::json!({}));
        }
    }
    pub fn alpha(&self) -> f32 {
        (self.accumulator / STEP).clamp(0.0, 1.0)
    }
    pub fn blocked(&self, p: Vec2, r: f32) -> bool {
        !self.terrain.bounds(p, r) || self.walls.iter().any(|w| w.solid(p, r))
    }
    /// Collision/attack ray against authoritative solids. Camera fade never enters this path.
    pub fn ray3(&self, a: Vec3, b: Vec3) -> Option<(usize, Vec3, f32)> {
        let steps = (a.distance(b) / 0.07).ceil().max(1.0) as usize;
        for i in 0..=steps {
            let f = i as f32 / steps as f32;
            let p = a.lerp(b, f);
            if self.terrain.solid(p) {
                return Some((self.walls.len(), p, f));
            }
            if let Some(w) = self.walls.iter().position(|w| w.solid3(p)) {
                return Some((w, p, f));
            }
        }
        None
    }
    pub fn body_point(&self, p: Vec2) -> Vec3 {
        let y = if p == self.player {
            self.player_body.y
        } else {
            self.enemies
                .iter()
                .find(|e| e.pos == p)
                .map_or(self.terrain.height(p), |e| e.body.y)
        };
        crate::controller::point(p, y + 0.85)
    }
    pub fn obstruction(&self, a: Vec2, b: Vec2) -> Option<(usize, Vec2, f32)> {
        self.ray3(self.body_point(a), self.body_point(b))
            .map(|(w, p, t)| (w, Vec2::new(p.x, p.z), t))
    }
    pub fn target(&self, range: f32) -> Option<u64> {
        self.enemies
            .iter()
            .filter(|e| {
                e.hp > 0.0
                    && crate::controller::point(e.pos, e.body.y + 0.85)
                        .distance(self.body_point(self.player))
                        <= range
                    && self.obstruction(self.player, e.pos).is_none()
            })
            .min_by(|a, b| {
                a.pos
                    .distance_squared(self.player)
                    .total_cmp(&b.pos.distance_squared(self.player))
                    .then(a.id.cmp(&b.id))
            })
            .map(|e| e.id)
    }
    pub fn observe(&self) -> serde_json::Value {
        serde_json::json!({"phase":self.phase,"player":self.player,"hp":self.hp,"currency":self.currency,"wave":self.wave,"scale":self.scale,"build":self.build,"offers":self.shop.offers,"walls":self.walls,"enemies":self.enemies.iter().filter(|e|e.pos.distance(self.player)<12.0 && self.obstruction(self.player,e.pos).is_none()).collect::<Vec<_>>(),"projectiles":self.shots.iter().filter(|s|s.pos.distance(self.player)<12.0 && self.obstruction(self.player,s.pos).is_none()).collect::<Vec<_>>()})
    }
    fn tick(&mut self, input: Vec2) {
        let stats = self.stats();
        self.world += STEP;
        self.wave_time += STEP;
        self.prev_player = self.player;
        let grounded = self.player_body.grounded;
        crate::controller::advance(
            &self.terrain,
            &self.walls,
            &self.physics,
            &mut self.player,
            &mut self.player_body,
            input * stats.speed * STEP,
            STEP,
        );
        if grounded && !self.player_body.grounded {
            self.metrics.falls += 1;
        }
        if input.length_squared() > 0.01
            && self.player.distance_squared(self.prev_player) < 0.000001
        {
            self.metrics.no_progress_seconds += STEP;
        }
        if self.player_body.stalled >= 2.0 {
            self.metrics.stuck_reports += 1;
            self.player_body.stalled = 0.0;
            self.event("stuck", serde_json::json!({"actor":"player"}));
        }
        let before = std::time::Instant::now();
        self.navigation
            .update(&self.terrain, &self.walls, &self.physics, self.player, STEP);
        self.metrics.navigation_update_us += before.elapsed().as_micros() as u64;
        self.metrics.navigation_rebuilds = self.navigation.rebuilds;
        if input.length_squared() > 0.001 {
            self.facing = input.x.atan2(input.y);
            self.moving += STEP;
            self.stationary = 0.0;
            if self.moving >= 2.0 {
                self.momentum = (self.momentum + STEP).min(1.0);
            }
        } else {
            self.stationary += STEP;
            self.moving = 0.0;
            self.momentum = (self.momentum - STEP).max(0.0);
            if self.stationary >= 0.75 {
                self.inertia = true;
            }
        }
        self.hp = (self.hp + stats.regen * STEP).min(stats.hp);
        self.invulnerability = (self.invulnerability - STEP).max(0.0);
        self.spawn -= STEP;
        if self.spawn <= 0.0 && self.enemies.len() < 300 {
            self.spawn = self.config.spawn_seconds / (1.0 + 0.16 * (self.wave - 1) as f32);
            let angle = self.combat_rng.unit() * std::f32::consts::TAU;
            let mut pos = self.player + Vec2::new(angle.cos(), angle.sin()) * 13.5;
            for attempt in 1..9 {
                if !self.blocked(pos, 0.4) {
                    break;
                }
                let angle = angle + attempt as f32 * 2.399963;
                pos = self.player + Vec2::new(angle.cos(), angle.sin()) * 13.5;
            }
            pos = pos.clamp(
                Vec2::splat(-crate::terrain::HALF + 1.5),
                Vec2::splat(crate::terrain::HALF - 1.5),
            );
            let kind = if self.wave == self.config.waves && !self.elite_spawned {
                self.elite_spawned = true;
                3
            } else if self.wave > 2 {
                self.combat_rng.index(3) as u32
            } else {
                0
            };
            self.enemies.push(Enemy {
                body: crate::controller::Body::at(crate::controller::support(
                    &self.terrain,
                    pos,
                    self.physics.radius,
                )),
                id: self.next_id,
                pos,
                prev: pos,
                hp: self.config.enemy_hp
                    * (1.0 + 0.12 * (self.wave - 1) as f32)
                    * if kind == 3 { 5.0 } else { 1.0 },
                kind,
                clock: 0.0,
                direction: Vec2::ZERO,
                chipped: 0,
            });
            self.next_id += 1;
        }
        // Move via axis sliding; blocked enemies chip their first obstruction with a bounded cadence.
        let mut demolish = Vec::new();
        let mut hostile = Vec::new();
        for i in 0..self.enemies.len() {
            let mut e = self.enemies[i].clone();
            e.prev = e.pos;
            e.clock += STEP;
            let direct = (self.player - e.pos).normalize_or_zero();
            let route = self.navigation.direction(e.pos, self.player);
            let toward = route.unwrap_or(Vec2::ZERO);
            if e.kind != 1 {
                e.direction = toward;
            }
            let mut velocity = toward * (1.4 + 0.08 * self.wave as f32);
            if e.kind == 1 {
                if e.clock < 0.7 {
                    velocity = Vec2::ZERO;
                    e.direction = toward;
                } else if e.clock < 1.3 {
                    velocity = e.direction * 7.0;
                } else {
                    e.clock = 0.0;
                }
            }
            if e.kind == 2
                && e.pos.distance(self.player) < 8.0
                && self.obstruction(e.pos, self.player).is_none()
            {
                velocity = Vec2::ZERO;
                if e.clock > 2.5 && self.obstruction(e.pos, self.player).is_none() {
                    let aim = (self.body_point(self.player)
                        - crate::controller::point(e.pos, e.body.y + 0.85))
                    .normalize_or_zero()
                        * 4.5;
                    hostile.push(Shot {
                        y: e.body.y + 0.85,
                        prev_y: e.body.y + 0.85,
                        velocity_y: aim.y,
                        pos: e.pos,
                        prev: e.pos,
                        velocity: Vec2::new(aim.x, aim.z),
                        life: 5.0,
                        damage: self.config.contact_damage,
                        hostile: true,
                        pierce: 0,
                        hit: Vec::new(),
                        primary: false,
                    });
                    e.clock = 0.0;
                }
            }
            let old = e.pos;
            let prior_stall = e.body.stalled;
            let was_grounded = e.body.grounded;
            crate::controller::advance(
                &self.terrain,
                &self.walls,
                &self.physics,
                &mut e.pos,
                &mut e.body,
                velocity * STEP,
                STEP,
            );
            let next = e.pos;
            if was_grounded && !e.body.grounded {
                self.metrics.falls += 1;
            }
            if route.is_none() {
                e.body.stalled = prior_stall + STEP;
            }
            if e.body.stalled >= 3.0 {
                self.metrics.unreachable_reports += 1;
                self.event("unreachable", serde_json::json!({"id":e.id,"pos":e.pos}));
                e.body.stalled = 0.0;
            }
            if next.distance_squared(old) < 0.000001
                && e.clock > 0.8
                && let Some((w, p, _)) = self.ray3(
                    crate::controller::point(e.pos, e.body.y + 0.8),
                    crate::controller::point(e.pos + direct * 0.8, e.body.y + 0.8),
                )
            {
                demolish.push((w, p));
                e.clock = 0.0;
            }
            e.pos = next;
            if e.pos.distance(self.player) < 0.65
                && (e.body.y - self.player_body.y).abs() < 0.7
                && self.obstruction(e.pos, self.player).is_none()
            {
                self.hurt(self.config.contact_damage);
            }
            self.enemies[i] = e;
        }
        self.shots.extend(hostile);
        for (w, p) in demolish {
            self.chip_wall_at(w, p, 0.35);
        }
        for slot in 0..3 {
            self.cooldown[slot] = (self.cooldown[slot] - STEP).max(0.0);
            let id = ["needle", "pulse", "satellite"][slot];
            let level = self.level(id);
            if level == 0 || self.cooldown[slot] > 0.0 {
                continue;
            }
            let range = if slot == 1 {
                2.7 * stats.area
            } else {
                stats.range
            };
            let target = self
                .target(range)
                .and_then(|id| self.enemies.iter().find(|e| e.id == id).map(|e| e.pos));
            let covered = self
                .enemies
                .iter()
                .filter(|e| e.pos.distance(self.player) <= range)
                .min_by(|a, b| {
                    a.pos
                        .distance_squared(self.player)
                        .total_cmp(&b.pos.distance_squared(self.player))
                        .then(a.id.cmp(&b.id))
                })
                .map(|e| e.pos);
            if target.is_none() && covered.is_none() {
                continue;
            }
            let mut damage =
                [12.0, 10.0, 8.0][slot] * (1.0 + 0.35 * (level - 1) as f32) * stats.damage;
            if self.inertia && input.length_squared() > 0.001 {
                damage *= 1.0 + 0.4 * self.level("inertia") as f32;
                self.inertia = false;
            }
            if self.combat_rng.unit() < stats.crit {
                damage *= stats.crit_multiplier;
            }
            self.activations[slot] += 1;
            let echo = self.level("echo");
            let repeat = echo > 0 && self.activations[slot].is_multiple_of(5);
            self.cooldown[slot] = [0.5, 1.15, 0.3][slot] / stats.attack_speed;
            for secondary in 0..=u32::from(repeat) {
                let power = if secondary == 0 {
                    damage
                } else {
                    damage * (0.15 + 0.15 * echo as f32)
                };
                if slot == 0 {
                    let p = target.or(covered).unwrap();
                    let direction = (self.body_point(p) - self.body_point(self.player))
                        .normalize_or_zero()
                        * 18.0;
                    self.shots.push(Shot {
                        y: self.player_body.y + 0.85,
                        prev_y: self.player_body.y + 0.85,
                        velocity_y: direction.y,
                        pos: self.player,
                        prev: self.player,
                        velocity: Vec2::new(direction.x, direction.z),
                        life: 1.2,
                        damage: power,
                        hostile: false,
                        pierce: self.level("piercing"),
                        hit: Vec::new(),
                        primary: secondary == 0,
                    });
                } else if slot == 1 {
                    self.area_damage(self.player, 2.7 * stats.area, power, secondary == 0);
                    let hits: Vec<_> = self
                        .walls
                        .iter()
                        .enumerate()
                        .filter_map(|(w, wall)| {
                            wall.cells
                                .iter()
                                .enumerate()
                                .find(|(i, v)| {
                                    **v && wall.center(*i).distance(self.body_point(self.player))
                                        < 2.7 * stats.area
                                })
                                .map(|(i, _)| (w, wall.center(i)))
                        })
                        .collect();
                    for (w, p) in hits {
                        self.chip_wall_at(w, p, 0.45);
                    }
                } else {
                    let orbit = self.orbit();
                    let victims: Vec<_> = self
                        .enemies
                        .iter()
                        .filter(|e| {
                            crate::controller::point(e.pos, e.body.y + 0.85)
                                .distance(crate::controller::point(orbit, self.player_body.y + 0.8))
                                < 0.8
                                && self.obstruction(self.player, e.pos).is_none()
                        })
                        .map(|e| e.id)
                        .collect();
                    for id in victims {
                        self.damage(id, power, secondary == 0);
                    }
                    let orb = crate::controller::point(orbit, self.player_body.y + 0.8);
                    if let Some(w) = self.walls.iter().position(|w| w.solid3(orb)) {
                        self.chip_wall_at(w, orb, 0.3);
                    }
                }
            }
        }
        self.grenade_timer = (self.grenade_timer - STEP).max(0.0);
        if self.level("grenade") > 0
            && self.grenade_timer <= 0.0
            && self.grenades.len() < 8
            && let Some(e) = self
                .enemies
                .iter()
                .filter(|e| e.hp > 0.0 && e.pos.distance(self.player) < stats.range)
                .min_by(|a, b| {
                    a.pos
                        .distance_squared(self.player)
                        .total_cmp(&b.pos.distance_squared(self.player))
                })
        {
            let start = crate::controller::point(self.player, self.player_body.y + 1.1);
            let end = crate::controller::point(e.pos, e.body.y + 0.1);
            let time = 0.7;
            let mut velocity = (end - start) / time;
            velocity.y += 0.5 * self.physics.gravity * time;
            self.grenades.push(Grenade {
                pos: start,
                prev: start,
                velocity,
                fuse: 1.4,
                damage: 18.0 * stats.damage * (1.0 + 0.35 * (self.level("grenade") - 1) as f32),
                radius: self.physics.grenade_radius * stats.area,
                strength: self.physics.terrain_strength,
            });
            self.grenade_timer = self.physics.grenade_cooldown / stats.attack_speed;
            self.event("grenade", serde_json::json!({"target":e.id}));
        }
        let grenades = std::mem::take(&mut self.grenades);
        for mut g in grenades {
            g.prev = g.pos;
            g.velocity.y -= self.physics.gravity * STEP;
            g.pos += g.velocity * STEP;
            g.fuse -= STEP;
            if let Some((_, p, _)) = self.ray3(g.prev, g.pos) {
                g.pos = p;
                g.fuse = 0.0;
            }
            if g.fuse <= 0.0 {
                self.explode(g.pos, g.radius, g.strength, g.damage);
            } else {
                self.grenades.push(g);
            }
        }
        let shots = std::mem::take(&mut self.shots);
        let grid =
            crate::spatial::Grid::new(self.enemies.iter().enumerate().map(|(i, e)| (i, e.pos)));
        for mut s in shots {
            s.prev = s.pos;
            s.prev_y = s.y;
            s.y += s.velocity_y * STEP;
            s.pos += s.velocity * STEP;
            s.life -= STEP;
            let obstruction = self.ray3(
                crate::controller::point(s.prev, s.prev_y),
                crate::controller::point(s.pos, s.y),
            );
            let cover_t = obstruction.map_or(2.0, |(_, _, t)| t);
            let mut consumed = false;
            if s.hostile {
                if segment_hit3(
                    crate::controller::point(s.prev, s.prev_y),
                    crate::controller::point(s.pos, s.y),
                    self.body_point(self.player),
                    0.5,
                )
                .is_some_and(|t| t < cover_t)
                {
                    self.hurt(s.damage);
                    consumed = true;
                }
            } else {
                let candidates = grid.segment(s.prev, s.pos, 0.5);
                let mut hits: Vec<_> = candidates
                    .iter()
                    .map(|&i| &self.enemies[i])
                    .filter(|e| e.hp > 0.0 && !s.hit.contains(&e.id))
                    .filter_map(|e| {
                        segment_hit3(
                            crate::controller::point(s.prev, s.prev_y),
                            crate::controller::point(s.pos, s.y),
                            crate::controller::point(e.pos, e.body.y + 0.85),
                            0.5,
                        )
                        .filter(|t| *t < cover_t)
                        .map(|t| (t, e.id))
                    })
                    .collect();
                hits.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
                for (_, id) in hits {
                    self.damage(id, s.damage, s.primary);
                    s.hit.push(id);
                    if s.pierce == 0 {
                        consumed = true;
                        break;
                    }
                    s.pierce -= 1;
                }
            }
            if !consumed && let Some((w, p, _)) = obstruction {
                if !s.hostile {
                    self.chip_wall_at(w, p, 0.32);
                }
                consumed = true;
            }
            if !consumed && s.life > 0.0 && self.shots.len() < 1000 {
                self.shots.push(s);
            }
        }
        // Unseen stragglers recycle without rewards; retain the final elite.
        self.enemies.retain(|e| {
            e.hp > 0.0 && (e.kind == 3 || e.pos.distance_squared(self.player) < 80.0 * 80.0)
        });
        let mut collected = 0;
        self.loot.retain(|p| {
            if p.distance(self.player) < stats.pickup
                && (self.terrain.height(*p) - self.player_body.y).abs() < 1.0
            {
                collected += 1;
                false
            } else {
                true
            }
        });
        self.credit(collected * 5);
        for d in &mut self.debris {
            d.prev = d.pos;
            d.velocity.y -= 9.8 * STEP;
            d.pos += d.velocity * STEP;
            d.life -= STEP;
            let floor = self.terrain.height(Vec2::new(d.pos.x, d.pos.z)) + 0.05;
            if d.pos.y < floor {
                d.pos.y = floor;
                d.velocity.y = d.velocity.y.abs() * 0.25;
                d.velocity.x *= 0.9;
                d.velocity.z *= 0.9;
            }
        }
        self.debris.retain(|d| d.life > 0.0);
        if self.hp <= 0.0 {
            self.phase = Phase::Dead;
            self.event("death", serde_json::json!({}));
        } else if self.wave_time >= self.config.wave_seconds
            && !(self.wave == self.config.waves && self.enemies.iter().any(|e| e.kind == 3))
        {
            self.credit(self.loot.len() as u32 * 5);
            self.loot.clear();
            self.enemies.clear();
            self.shots.clear();
            self.grenades.clear();
            self.accumulator = 0.0;
            self.metrics.snapshots.push(serde_json::json!({"wave":self.wave,"hp":self.hp,"world":self.world,"real":self.real,"currency":self.currency,"kills":self.metrics.kills}));
            if self.wave == self.config.waves {
                self.phase = Phase::Victory;
            } else {
                self.phase = Phase::Shop;
                self.shop.rerolls = 0;
                self.fill_shop();
            }
            self.event("wave_end", serde_json::json!({"phase":self.phase}));
        }
    }
    pub fn orbit(&self) -> Vec2 {
        self.player
            + Vec2::new((self.world * 2.5).cos(), (self.world * 2.5).sin())
                * 2.2
                * self.stats().area
    }
    pub fn start_wave(&mut self) -> Result<(), String> {
        if self.phase != Phase::Shop {
            return Err("Not in shop".into());
        }
        self.wave += 1;
        self.wave_time = 0.0;
        self.spawn = 0.0;
        self.accumulator = 0.0;
        self.phase = Phase::Combat;
        self.event("wave_start", serde_json::json!({}));
        Ok(())
    }
    fn hurt(&mut self, damage: f32) {
        if self.invulnerability > 0.0 {
            return;
        }
        let armor = self.stats().armor;
        let d = if armor >= 0.0 {
            damage * 10.0 / (10.0 + armor)
        } else {
            damage * (1.0 - armor / 10.0)
        };
        self.hp -= d;
        self.metrics.received += d;
        self.invulnerability = 0.7;
        self.event("hurt", serde_json::json!({"damage":d}));
    }
    fn credit(&mut self, n: u32) {
        if n > 0 {
            self.currency += n;
            self.metrics.earned += n;
            self.event("earn", serde_json::json!({"coins":n}));
        }
    }
    fn area_damage(&mut self, p: Vec2, r: f32, damage: f32, primary: bool) {
        let ids: Vec<_> = self
            .enemies
            .iter()
            .filter(|e| {
                e.hp > 0.0
                    && self.body_point(e.pos).distance(self.body_point(p)) < r
                    && self.obstruction(p, e.pos).is_none()
            })
            .map(|e| e.id)
            .collect();
        for id in ids {
            self.damage(id, damage, primary);
        }
    }
    fn damage(&mut self, id: u64, amount: f32, primary: bool) {
        let Some(i) = self.enemies.iter().position(|e| e.id == id && e.hp > 0.0) else {
            return;
        };
        let pos = self.enemies[i].pos;
        self.enemies[i].hp -= amount;
        self.enemies[i].chipped = (self.enemies[i].chipped + 1).min(4);
        self.metrics.dealt += amount;
        self.event(
            "hit",
            serde_json::json!({"target":id,"damage":amount,"primary":primary}),
        );
        let dead = self.enemies[i].hp <= 0.0;
        self.emit(
            Vec3::new(pos.x, self.enemies[i].body.y + 0.8, pos.y),
            if dead { 36 } else { 4 },
            [0.7, 0.35, 0.4],
        );
        if dead {
            self.metrics.kills += 1;
            self.loot.push(pos);
            self.event("kill", serde_json::json!({"id":id}));
            if primary && self.level("rupture") > 0 {
                self.area_damage(pos, 1.8, 5.0 * self.level("rupture") as f32, false);
            }
        }
        if primary
            && self.level("chain") > 0
            && let Some(other) = self
                .enemies
                .iter()
                .filter(|e| {
                    e.id != id
                        && e.hp > 0.0
                        && e.pos.distance(pos) < 2.5
                        && self.obstruction(pos, e.pos).is_none()
                })
                .min_by(|a, b| {
                    a.pos
                        .distance_squared(pos)
                        .total_cmp(&b.pos.distance_squared(pos))
                        .then(a.id.cmp(&b.id))
                })
                .map(|e| e.id)
        {
            self.damage(other, 3.0 * self.level("chain") as f32, false);
        }
    }
    /// Terrain and collision become authoritative atomically on this simulation tick.
    /// Renderer consumes checksum-stamped chunks synchronously before presenting.
    pub fn explode(&mut self, center: Vec3, radius: f32, strength: f32, damage: f32) {
        let victims: Vec<_> = self
            .enemies
            .iter()
            .filter(|e| {
                damage > 0.0
                    && e.hp > 0.0
                    && self.body_point(e.pos).distance(center) < radius
                    && self
                        .ray3(center + Vec3::Y * 0.12, self.body_point(e.pos))
                        .is_none()
            })
            .map(|e| e.id)
            .collect();
        for id in victims {
            self.damage(id, damage, true);
        }
        let start = std::time::Instant::now();
        let edit = self.terrain.excavate(center, radius, strength);
        self.metrics.terrain_update_us += start.elapsed().as_micros() as u64;
        if edit.removed > 0 {
            self.metrics.terrain_edits += 1;
            self.metrics.removed_voxels += edit.removed as u64;
            self.metrics.dirty_chunks += edit.dirty_chunks as u64;
            self.event("excavation",serde_json::json!({"center":center,"radius":radius,"strength":strength,"removed":edit.removed,"revision":self.terrain.revision}));
            for p in edit.fragments {
                self.emit(p, 1, [0.47, 0.34, 0.22]);
            }
            self.metrics.falls += crate::controller::reconcile(
                &self.terrain,
                &self.walls,
                self.player,
                &self.physics,
                &mut self.player_body,
            ) as u32;
            for e in &mut self.enemies {
                self.metrics.falls += crate::controller::reconcile(
                    &self.terrain,
                    &self.walls,
                    e.pos,
                    &self.physics,
                    &mut e.body,
                ) as u32;
            }
        }
        for w in 0..self.walls.len() {
            self.chip_wall_at(w, center, radius * 0.6);
        }
    }
    pub fn chip_wall(&mut self, w: usize, p: Vec2, r: f32) {
        self.chip_wall_at(w, crate::controller::point(p, 0.8), r);
    }
    pub fn chip_wall_at(&mut self, w: usize, p: Vec3, r: f32) {
        let Some(wall) = self.walls.get_mut(w) else {
            return;
        };
        let mut removed = Vec::new();
        for i in 0..wall.cells.len() {
            let c = wall.center(i);
            if wall.cells[i] && c.distance(p) < r {
                wall.cells[i] = false;
                removed.push(c);
            }
        }
        if removed.is_empty() {
            return;
        }
        removed.extend(wall.support());
        self.metrics.cover_removed += removed.len() as u32;
        self.event(
            "destruction",
            serde_json::json!({"wall":w,"removed":removed.len()}),
        );
        for c in removed.iter().step_by((removed.len() / 48).max(1)).take(48) {
            self.emit(*c, 1, [0.55, 0.61, 0.62]);
        }
        self.metrics.falls += crate::controller::reconcile(
            &self.terrain,
            &self.walls,
            self.player,
            &self.physics,
            &mut self.player_body,
        ) as u32;
        for e in &mut self.enemies {
            self.metrics.falls += crate::controller::reconcile(
                &self.terrain,
                &self.walls,
                e.pos,
                &self.physics,
                &mut e.body,
            ) as u32;
        }
    }
    fn emit(&mut self, pos: Vec3, count: usize, color: [f32; 3]) {
        for _ in 0..count {
            let velocity = Vec3::new(
                (self.visual_rng.unit() - 0.5) * 4.0,
                2.0 + self.visual_rng.unit() * 3.0,
                (self.visual_rng.unit() - 0.5) * 4.0,
            );
            self.debris.push(Debris {
                pos,
                prev: pos,
                velocity,
                life: 3.0,
                color,
            });
        }
        let overflow = self.debris.len().saturating_sub(self.config.debris_budget);
        if overflow > 0 {
            self.debris.drain(..overflow);
        }
        self.metrics.debris_peak = self.metrics.debris_peak.max(self.debris.len());
    }
    pub fn save(&self, path: &std::path::Path) -> Result<(), String> {
        if self.phase != Phase::Shop {
            return Err("Only shop checkpoints are supported".into());
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self).unwrap())
            .map_err(|e| e.to_string())?;
        // Windows rename cannot replace an existing file. ReplaceFileW preserves atomicity.
        #[cfg(windows)]
        if path.exists() {
            use std::os::windows::ffi::OsStrExt;
            unsafe extern "system" {
                fn ReplaceFileW(
                    replaced: *const u16,
                    replacement: *const u16,
                    backup: *const u16,
                    flags: u32,
                    exclude: *mut std::ffi::c_void,
                    reserved: *mut std::ffi::c_void,
                ) -> i32;
            }
            let a: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
            let b: Vec<u16> = tmp.as_os_str().encode_wide().chain(Some(0)).collect();
            // SAFETY: both paths are owned NUL-terminated UTF-16 buffers for the duration of this call.
            if unsafe {
                ReplaceFileW(
                    a.as_ptr(),
                    b.as_ptr(),
                    std::ptr::null(),
                    0,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            } == 0
            {
                return Err(std::io::Error::last_os_error().to_string());
            }
            return Ok(());
        }
        std::fs::rename(tmp, path).map_err(|e| e.to_string())
    }
    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        let mut g: Self = serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        if ![1, 2].contains(&g.schema) || g.phase != Phase::Shop {
            return Err("Unsupported checkpoint".into());
        }
        if g.schema == 1 {
            g.terrain = crate::terrain::Terrain::new(g.seed);
            g.player_body = crate::controller::Body::at(crate::controller::support(
                &g.terrain,
                g.player,
                g.physics.radius,
            ));
            g.schema = 2;
        }
        g.terrain.upgrade_resolution()?;
        if !g.terrain.validate()
            || !g.physics.validate()
            || !g.terrain.bounds(g.player, g.physics.radius)
            || g.terrain.seed != g.seed
            || !g.player.is_finite()
            || !g.player_body.y.is_finite()
            || !g.player_body.velocity.is_finite()
            || !g.player_body.visual_y.is_finite()
            || !(crate::terrain::BOTTOM + crate::terrain::Y - 0.02..12.0).contains(&g.player_body.y)
        {
            return Err("Invalid voxel checkpoint".into());
        }
        g.config.validate()?;
        g.cards = crate::content::load();
        if g.walls.iter().any(|w| {
            !w.origin.is_finite()
                || w.size.iter().any(|&n| n == 0 || n > 100)
                || w.size.iter().try_fold(1usize, |n, &v| n.checked_mul(v)) != Some(w.cells.len())
        }) || g.build.iter().any(|o| {
            !g.cards
                .iter()
                .any(|c| c.id == o.id && o.level > 0 && o.level <= c.max_level)
        }) {
            return Err("Invalid checkpoint content".into());
        }
        g.debris.clear();
        g.events.clear();
        Ok(g)
    }
}
fn segment_hit3(a: Vec3, b: Vec3, p: Vec3, r: f32) -> Option<f32> {
    let d = b - a;
    let o = a - p;
    let c = o.length_squared() - r * r;
    if c <= 0.0 {
        return Some(0.0);
    }
    let aa = d.length_squared();
    if aa < 1e-9 {
        return None;
    }
    let bb = o.dot(d);
    let disc = bb * bb - aa * c;
    if disc < 0.0 {
        return None;
    }
    let t = (-bb - disc.sqrt()) / aa;
    if (0.0..=1.0).contains(&t) {
        Some(t)
    } else {
        None
    }
}
