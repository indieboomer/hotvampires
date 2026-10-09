//! Seeded profile plus authoritative finite voxel soil. Edits are synchronous.
//! Bits, cached column surfaces and neighboring mesh stamps change together.
use glam::{Vec2, Vec3};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

pub const MESH_STEP: i32 = 2;
#[derive(Deserialize)]
pub struct Settings {
    pub patch_spacing: f32,
    pub noise_wavelength: f32,
    pub height_amplitude: f32,
    pub detail_amplitude: f32,
    pub flat_fraction: f32,
    pub starting_clearing_radius: f32,
    pub starting_blend_radius: f32,
}

pub const XZ: f32 = 0.5;
pub const Y: f32 = 0.25;
pub const BOTTOM: f32 = -8.0;
pub const COLS: usize = 128;
pub const LAYERS: usize = 64;
pub const CHUNK_CELLS: usize = 16;
pub const CHUNKS: usize = COLS / CHUNK_CELLS;
pub const HALF: f32 = COLS as f32 * XZ * 0.5;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Physics {
    pub step_height: f32,
    pub max_slope_degrees: f32,
    pub ground_snap: f32,
    pub gravity: f32,
    pub radius: f32,
    pub body_height: f32,
    pub grenade_radius: f32,
    pub terrain_strength: f32,
    pub grenade_cooldown: f32,
    pub camera_follow_seconds: f32,
    pub camera_fade_seconds: f32,
}
impl Default for Physics {
    fn default() -> Self {
        let p: Self = serde_json::from_str(include_str!("../data/voxel.json")).unwrap();
        assert!(p.validate());
        p
    }
}
impl Physics {
    pub fn validate(&self) -> bool {
        [
            self.step_height,
            self.ground_snap,
            self.max_slope_degrees,
            self.gravity,
            self.radius,
            self.body_height,
            self.grenade_radius,
            self.terrain_strength,
            self.grenade_cooldown,
            self.camera_follow_seconds,
            self.camera_fade_seconds,
        ]
        .iter()
        .all(|v| v.is_finite())
            && (Y..=1.0).contains(&self.step_height)
            && (0.0..=self.step_height).contains(&self.ground_snap)
            && (1.0..80.0).contains(&self.max_slope_degrees)
            && (1.0..50.0).contains(&self.gravity)
            && (0.1..0.6).contains(&self.radius)
            && self.body_height > self.radius * 2.0
            && self.body_height < 3.0
            && (0.25..=8.0).contains(&self.grenade_radius)
            && (0.01..=3.0).contains(&self.terrain_strength)
            && self.grenade_cooldown >= 0.1
            && self.camera_follow_seconds >= 0.01
            && self.camera_fade_seconds >= 0.01
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Chunk {
    pub bits: Vec<u8>,
    pub checksum: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Terrain {
    pub seed: u64,
    pub revision: u64,
    pub chunks: Vec<Chunk>,
    pub tops: Vec<i16>,
}
#[derive(Default, Debug)]
pub struct Edit {
    pub removed: usize,
    pub fragments: Vec<Vec3>,
    pub dirty_chunks: usize,
}
impl Default for Terrain {
    fn default() -> Self {
        Self::new(1)
    }
}
impl Terrain {
    pub fn digest(&self) -> u64 {
        self.chunks
            .iter()
            .flat_map(|c| c.bits.iter())
            .fold(0xcbf29ce484222325, |h, &b| {
                (h ^ b as u64).wrapping_mul(0x100000001b3)
            })
    }
    pub fn new(seed: u64) -> Self {
        let mut t = Self {
            seed,
            revision: 0,
            chunks: vec![
                Chunk {
                    bits: vec![0; CHUNK_CELLS * CHUNK_CELLS * LAYERS / 8],
                    checksum: 0
                };
                CHUNKS * CHUNKS
            ],
            tops: vec![0; COLS * COLS],
        };
        for z in 0..COLS {
            for x in 0..COLS {
                let p = Self::column_center(x, z);
                let surface = if Self::protected(x, z) {
                    2.5
                } else {
                    height(seed, p).clamp(-3.5, 3.5)
                };
                let top = ((surface - BOTTOM) / Y).ceil() as i16 - 1;
                t.tops[x + COLS * z] = top;
                for y in 0..=top as usize {
                    t.set_raw(x, y, z, true);
                }
            }
        }
        for chunk in &mut t.chunks {
            chunk.checksum = checksum(&chunk.bits);
        }
        t
    }
    pub fn protected(x: usize, z: usize) -> bool {
        x == 0 || z == 0 || x == COLS - 1 || z == COLS - 1
    }
    pub fn column_center(x: usize, z: usize) -> Vec2 {
        Vec2::new((x as f32 + 0.5) * XZ - HALF, (z as f32 + 0.5) * XZ - HALF)
    }
    pub fn column(p: Vec2) -> Option<(usize, usize)> {
        let x = ((p.x + HALF) / XZ).floor() as isize;
        let z = ((p.y + HALF) / XZ).floor() as isize;
        if x < 0 || z < 0 || x >= COLS as isize || z >= COLS as isize {
            None
        } else {
            Some((x as usize, z as usize))
        }
    }
    fn address(x: usize, y: usize, z: usize) -> (usize, usize) {
        let chunk = x / CHUNK_CELLS + CHUNKS * (z / CHUNK_CELLS);
        let bit = x % CHUNK_CELLS + CHUNK_CELLS * (z % CHUNK_CELLS + CHUNK_CELLS * y);
        (chunk, bit)
    }
    fn set_raw(&mut self, x: usize, y: usize, z: usize, value: bool) {
        let (c, b) = Self::address(x, y, z);
        if value {
            self.chunks[c].bits[b / 8] |= 1 << (b % 8);
        } else {
            self.chunks[c].bits[b / 8] &= !(1 << (b % 8));
        }
    }
    pub fn cell(&self, x: isize, y: isize, z: isize) -> bool {
        if x < 0
            || z < 0
            || y < 0
            || x >= COLS as isize
            || z >= COLS as isize
            || y >= LAYERS as isize
        {
            return false;
        }
        let (c, b) = Self::address(x as usize, y as usize, z as usize);
        self.chunks[c].bits[b / 8] & (1 << (b % 8)) != 0
    }
    pub fn solid(&self, p: Vec3) -> bool {
        let Some((x, z)) = Self::column(Vec2::new(p.x, p.z)) else {
            return false;
        };
        self.cell(
            x as isize,
            ((p.y - BOTTOM) / Y).floor() as isize,
            z as isize,
        )
    }
    pub fn top(&self, x: usize, z: usize) -> f32 {
        BOTTOM + (self.tops[x + COLS * z] + 1) as f32 * Y
    }
    pub fn height(&self, p: Vec2) -> f32 {
        Self::column(p).map_or(BOTTOM + Y, |(x, z)| self.top(x, z))
    }
    pub fn bounds(&self, p: Vec2, r: f32) -> bool {
        p.abs().max_element() < HALF - XZ - r
    }
    pub fn stamp(&self, cx: usize, cz: usize) -> u64 {
        let mut h = self.chunks[cx + CHUNKS * cz].checksum;
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
            let x = cx as i32 + dx;
            let z = cz as i32 + dz;
            if x >= 0 && z >= 0 && x < CHUNKS as i32 && z < CHUNKS as i32 {
                h = h.rotate_left(11) ^ self.chunks[x as usize + CHUNKS * z as usize].checksum;
            }
        }
        h
    }
    fn lower(
        &mut self,
        x: usize,
        z: usize,
        new_top: i16,
        edit: &mut Edit,
        dirty: &mut std::collections::BTreeSet<usize>,
    ) {
        if Self::protected(x, z) {
            return;
        }
        let old = self.tops[x + COLS * z];
        let new_top = new_top.max(0).min(old);
        if new_top == old {
            return;
        }
        let p = Self::column_center(x, z);
        for y in new_top + 1..=old {
            self.set_raw(x, y as usize, z, false);
            edit.removed += 1;
            if edit.fragments.len() < 64
                && (edit.removed.is_multiple_of(7) || edit.fragments.is_empty())
            {
                edit.fragments
                    .push(Vec3::new(p.x, BOTTOM + (y as f32 + 0.5) * Y, p.y));
            }
        }
        self.tops[x + COLS * z] = new_top;
        dirty.insert(x / CHUNK_CELLS + CHUNKS * (z / CHUNK_CELLS));
    }
    /// Approximately spherical (vertically scaled) excavation with real rim erosion.
    /// Removed soil caps collapse vertically, keeping this first version cave-free.
    pub fn excavate(&mut self, center: Vec3, radius: f32, strength: f32) -> Edit {
        let mut edit = Edit::default();
        if radius <= 0.0 || strength <= 0.0 || !center.is_finite() {
            return edit;
        }
        let r = radius.clamp(0.25, 8.0);
        let ry = (r * strength).clamp(Y, 6.0);
        let min = Vec2::new(center.x - r - 1.0, center.z - r - 1.0);
        let max = Vec2::new(center.x + r + 1.0, center.z + r + 1.0);
        let x0 = ((min.x + HALF) / XZ).floor().max(1.0) as usize;
        let z0 = ((min.y + HALF) / XZ).floor().max(1.0) as usize;
        let x1 = ((max.x + HALF) / XZ).ceil().clamp(1.0, (COLS - 2) as f32) as usize;
        let z1 = ((max.y + HALF) / XZ).ceil().clamp(1.0, (COLS - 2) as f32) as usize;
        let mut dirty = std::collections::BTreeSet::new();
        for z in z0..=z1 {
            for x in x0..=x1 {
                let p = Self::column_center(x, z);
                let horizontal = (p - Vec2::new(center.x, center.z)).length_squared() / (r * r);
                if horizontal >= 1.0 {
                    continue;
                }
                let extent = ry * (1.0 - horizontal).sqrt();
                let low = center.y - extent;
                let high = center.y + extent;
                let top = self.top(x, z);
                if high >= BOTTOM + Y && low < top && high > low {
                    let lowest = ((low - BOTTOM) / Y).floor() as i16;
                    if BOTTOM + (lowest.max(1) as f32 + 0.5) * Y <= high {
                        self.lower(x, z, lowest - 1, &mut edit, &mut dirty);
                    }
                }
            }
        }
        // Bevel an ordinary crater by removing additional rim voxels, never by adding ramps.
        // Bounded erosion region/passes means repeated deep pits can honestly be impassable.
        if edit.removed > 0 && strength <= 1.0 {
            for _ in 0..8 {
                let mut changes = Vec::new();
                for z in z0..=z1 {
                    for x in x0..=x1 {
                        let top = self.tops[x + COLS * z];
                        let mut allowed = top;
                        for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                            let nx = (x as i32 + dx) as usize;
                            let nz = (z as i32 + dz) as usize;
                            allowed = allowed.min(self.tops[nx + COLS * nz] + 2);
                        }
                        if allowed < top {
                            changes.push((x, z, allowed));
                        }
                    }
                }
                if changes.is_empty() {
                    break;
                }
                for (x, z, top) in changes {
                    self.lower(x, z, top, &mut edit, &mut dirty);
                }
            }
        }
        if edit.removed > 0 {
            self.revision += 1;
            for &c in &dirty {
                self.chunks[c].checksum = checksum(&self.chunks[c].bits);
            }
            edit.dirty_chunks = dirty.len();
        }
        edit
    }
    pub fn set_column(&mut self, p: Vec2, top: f32) {
        if let Some((x, z)) = Self::column(p) {
            let level = (((top - BOTTOM) / Y).round() as i16 - 1).clamp(0, LAYERS as i16 - 1);
            self.tops[x + COLS * z] = level;
            for y in 0..LAYERS {
                self.set_raw(x, y, z, y <= level as usize);
            }
            let (c, _) = Self::address(x, 0, z);
            self.chunks[c].checksum = checksum(&self.chunks[c].bits);
            self.revision += 1;
        }
    }
    pub fn validate(&self) -> bool {
        self.chunks.len() == CHUNKS * CHUNKS
            && self.tops.len() == COLS * COLS
            && self.chunks.iter().all(|c| {
                c.bits.len() == CHUNK_CELLS * CHUNK_CELLS * LAYERS / 8
                    && c.checksum == checksum(&c.bits)
            })
            && self.tops.iter().all(|&t| (0..LAYERS as i16).contains(&t))
            && (0..COLS * COLS).all(|i| {
                let x = i % COLS;
                let z = i / COLS;
                let top = self.tops[i] as usize;
                (!Self::protected(x, z) || self.top(x, z) == 2.5)
                    && (0..LAYERS)
                        .all(|y| self.cell(x as isize, y as isize, z as isize) == (y <= top))
            })
    }
}
fn checksum(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |h, &b| {
        (h ^ b as u64).wrapping_mul(0x100000001b3)
    })
}
pub fn settings() -> &'static Settings {
    static SETTINGS: OnceLock<Settings> = OnceLock::new();
    SETTINGS.get_or_init(|| {
        let s: Settings = serde_json::from_str(include_str!("../data/terrain.json"))
            .expect("invalid terrain settings");
        assert!(
            s.patch_spacing >= 24.0
                && s.noise_wavelength >= s.patch_spacing
                && s.height_amplitude >= 0.0
                && s.detail_amplitude >= 0.0
                && (0.1..0.45).contains(&s.flat_fraction)
                && s.starting_blend_radius > s.starting_clearing_radius
        );
        s
    })
}
fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
fn lattice(seed: u64, x: i64, z: i64) -> f32 {
    let mut h = seed
        ^ (x as u64).wrapping_mul(0x9e3779b97f4a7c15)
        ^ (z as u64).wrapping_mul(0xbf58476d1ce4e5b9);
    h = (h ^ (h >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94d049bb133111eb);
    h ^= h >> 31;
    (h >> 40) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0
}
pub fn noise(seed: u64, p: Vec2) -> f32 {
    let x = p.x.floor() as i64;
    let z = p.y.floor() as i64;
    let tx = smooth(p.x - p.x.floor());
    let tz = smooth(p.y - p.y.floor());
    let a = lattice(seed, x, z);
    let b = lattice(seed, x + 1, z);
    let c = lattice(seed, x, z + 1);
    let d = lattice(seed, x + 1, z + 1);
    let lower = a + (b - a) * tx;
    let upper = c + (d - c) * tx;
    lower + (upper - lower) * tz
}
fn pad_height(seed: u64, x: f32, z: f32) -> f32 {
    let s = settings();
    let p = Vec2::new(x, z) * s.patch_spacing;
    noise(seed, p / s.noise_wavelength) * s.height_amplitude
        + noise(seed ^ 0x77665544, p / (s.noise_wavelength * 0.5)) * s.detail_amplitude
}
/// Samples the shared vertices of the regular triangle mesh.
pub fn vertex_height(seed: u64, p: Vec2) -> f32 {
    let s = settings();
    let distance = p.abs().max_element();
    if distance <= s.starting_clearing_radius {
        return 0.0;
    }
    let grid = p / s.patch_spacing;
    let origin = grid.floor();
    let fraction = grid - origin;
    let width = 1.0 - 2.0 * s.flat_fraction;
    let tx = smooth((fraction.x - s.flat_fraction) / width);
    let tz = smooth((fraction.y - s.flat_fraction) / width);
    let a = pad_height(seed, origin.x, origin.y);
    let b = pad_height(seed, origin.x + 1.0, origin.y);
    let c = pad_height(seed, origin.x, origin.y + 1.0);
    let d = pad_height(seed, origin.x + 1.0, origin.y + 1.0);
    let lower = a + (b - a) * tx;
    let upper = c + (d - c) * tx;
    (lower + (upper - lower) * tz)
        * smooth(
            (distance - s.starting_clearing_radius)
                / (s.starting_blend_radius - s.starting_clearing_radius),
        )
}
/// Exact barycentric height on the rendered triangles, including negative coordinates.
pub fn height(seed: u64, p: Vec2) -> f32 {
    let step = MESH_STEP as f32;
    let grid = (p / step).floor() * step;
    let t = (p - grid) / step;
    let a = vertex_height(seed, grid);
    let b = vertex_height(seed, grid + Vec2::new(step, 0.0));
    let c = vertex_height(seed, grid + Vec2::new(0.0, step));
    let d = vertex_height(seed, grid + Vec2::splat(step));
    if t.x + t.y <= 1.0 {
        a + (b - a) * t.x + (c - a) * t.y
    } else {
        d + (c - d) * (1.0 - t.x) + (b - d) * (1.0 - t.y)
    }
}
