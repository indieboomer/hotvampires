//! Seeded, indestructible height-field terrain. No voxel occupancy or mutation.
//! Broad noise sets pad elevations; smooth transitions join genuinely flat pads.
use glam::Vec2;
use serde::Deserialize;
use std::sync::OnceLock;

pub const CHUNK_SIZE: i32 = 16;
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
