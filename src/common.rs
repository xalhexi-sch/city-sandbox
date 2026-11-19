use bevy::prelude::*;

// ---------------------------------------------------------------- layout ---
pub const N: i32 = 7; // blocks per axis
pub const BLOCK: f32 = 36.0;
pub const STREET: f32 = 16.0;
pub const PITCH: f32 = BLOCK + STREET;
pub const HALF: f32 = (N as f32) * PITCH * 0.5 + STREET * 0.5;
pub const CURB: f32 = 0.3;
pub const ROAD_Y: f32 = 0.06;
pub const LOT: f32 = 15.0;
pub const LOT_OFF: f32 = 8.5;

// ------------------------------------------------------------ components ---
#[derive(Component)]
pub struct Player;
#[derive(Component)]
pub struct MainCamera;
#[derive(Component)]
pub struct Sun;

// ------------------------------------------------------------- resources ---
#[derive(Resource, Default)]
pub struct Driving(pub Option<Entity>);

#[derive(Resource)]
pub struct Colliders(pub Vec<(Vec2, Vec2)>);

#[derive(Resource)]
pub struct BlockCenters(pub Vec<Vec2>);

/// Thin obstacles (lamp posts, signal poles, tree trunks). Kept apart from `Colliders`
/// because those are treated as full-height by the camera clipping.
#[derive(Resource, Default)]
pub struct Furniture(pub Vec<(Vec2, Vec2)>);

#[derive(Resource)]
pub struct Clock {
    pub hours: f32,
    pub rate: f32, // game-hours per real second
}

/// Traffic-light phase shared by every intersection.
#[derive(Resource, Default)]
pub struct Signals {
    pub t: f32,
}
impl Signals {
    pub const CYCLE: f32 = 28.0;
    /// 0 = green, 1 = yellow, 2 = red for the given axis (0 = north-south travel, 1 = east-west).
    pub fn state(&self, axis: usize) -> u8 {
        let t = (self.t + if axis == 0 { 0.0 } else { Self::CYCLE / 2.0 }) % Self::CYCLE;
        if t < 11.0 {
            0
        } else if t < 13.0 {
            1
        } else {
            2
        }
    }
}

#[derive(Resource)]
pub struct Rng(pub u64);
impl Rng {
    pub fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.next()
    }
    pub fn pick(&mut self, n: usize) -> usize {
        ((self.next() * n as f32) as usize).min(n - 1)
    }
    pub fn chance(&mut self, p: f32) -> bool {
        self.next() < p
    }
}

// --------------------------------------------------------------- helpers ---
pub fn ground_h(x: f32, z: f32) -> f32 {
    let on_block = |v: f32| {
        let i = (v / PITCH + (N - 1) as f32 / 2.0).round();
        let c = (i - (N - 1) as f32 / 2.0) * PITCH;
        i >= 0.0 && i < N as f32 && (v - c).abs() <= BLOCK / 2.0
    };
    if on_block(x) && on_block(z) {
        CURB
    } else {
        ROAD_Y
    }
}

/// Circle-vs-AABB push out on the XZ plane. Returns the (normalised) average push
/// direction, or ZERO when nothing was touched.
pub fn push_out(pos: &mut Vec3, r: f32, cols: &[(Vec2, Vec2)]) -> Vec2 {
    let mut n = Vec2::ZERO;
    for (c, h) in cols {
        let p = Vec2::new(pos.x, pos.z);
        if (p.x - c.x).abs() > h.x + r || (p.y - c.y).abs() > h.y + r {
            continue;
        }
        let closest = p.clamp(*c - *h, *c + *h);
        let d = p - closest;
        let d2 = d.length_squared();
        if d2 >= r * r {
            continue;
        }
        let push;
        if d2 > 1e-6 {
            let dist = d2.sqrt();
            push = d / dist * (r - dist);
        } else {
            let dx = h.x - (p.x - c.x).abs();
            let dz = h.y - (p.y - c.y).abs();
            push = if dx < dz {
                Vec2::new((p.x - c.x).signum() * (dx + r), 0.0)
            } else {
                Vec2::new(0.0, (p.y - c.y).signum() * (dz + r))
            };
        }
        pos.x += push.x;
        pos.z += push.y;
        n += push.normalize_or_zero();
    }
    n.normalize_or_zero()
}

pub fn clamp_world(pos: &mut Vec3) {
    let m = HALF - 2.0;
    pos.x = pos.x.clamp(-m, m);
    pos.z = pos.z.clamp(-m, m);
}

pub fn axis(keys: &ButtonInput<KeyCode>, pos: KeyCode, neg: KeyCode) -> f32 {
    (keys.pressed(pos) as i32 - keys.pressed(neg) as i32) as f32
}

/// Rotation that makes a -Z-forward object face along `d`.
pub fn yaw_facing(d: Vec3) -> Quat {
    Quat::from_rotation_y(f32::atan2(-d.x, -d.z))
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

pub fn smooth(rate: f32, dt: f32) -> f32 {
    1.0 - (-rate * dt).exp()
}

pub fn srgb(r: f32, g: f32, b: f32) -> Color {
    Color::srgb(r, g, b)
}

/// Position of the i-th street centre line.
pub fn street_c(j: i32) -> f32 {
    (j as f32 - N as f32 / 2.0) * PITCH
}
