//! Procedural humanoid: jointed body, detailed face, distance-driven walk/run cycle.
//! Convention: character faces -Z, +X is the character's right, feet at y = 0.

use crate::common::*;
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

#[derive(Resource)]
pub struct HumanAssets {
    pub sphere: Handle<Mesh>,
    pub cube: Handle<Mesh>,
    pub thigh: Handle<Mesh>,
    pub shin: Handle<Mesh>,
    pub upper_arm: Handle<Mesh>,
    pub forearm: Handle<Mesh>,
    pub chest: Handle<Mesh>,
    pub pelvis: Handle<Mesh>,
    pub shoe: Handle<Mesh>,
    pub neck: Handle<Mesh>,
    pub lip: Handle<Mesh>,
    pub ring: Handle<Mesh>,
    pub sclera: Handle<StandardMaterial>,
    pub pupil: Handle<StandardMaterial>,
    pub mouth: Handle<StandardMaterial>,
    pub frame: Handle<StandardMaterial>,
    pub sole: Handle<StandardMaterial>,
}

impl HumanAssets {
    pub fn new(meshes: &mut Assets<Mesh>, mats: &mut Assets<StandardMaterial>) -> Self {
        Self {
            sphere: meshes.add(Sphere::new(1.0).mesh().uv(32, 18)),
            cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
            thigh: meshes.add(Capsule3d::new(0.075, 0.30).mesh().rings(4).latitudes(10).longitudes(20)),
            shin: meshes.add(Capsule3d::new(0.055, 0.32).mesh().rings(4).latitudes(10).longitudes(20)),
            upper_arm: meshes.add(Capsule3d::new(0.045, 0.21).mesh().rings(3).latitudes(8).longitudes(16)),
            forearm: meshes.add(Capsule3d::new(0.038, 0.20).mesh().rings(3).latitudes(8).longitudes(16)),
            chest: meshes.add(Capsule3d::new(0.15, 0.20).mesh().rings(6).latitudes(12).longitudes(24)),
            pelvis: meshes.add(Capsule3d::new(0.1, 0.16).mesh().rings(4).latitudes(10).longitudes(20)),
            shoe: meshes.add(Capsule3d::new(0.045, 0.14).mesh().rings(3).latitudes(8).longitudes(16)),
            neck: meshes.add(Cylinder::new(0.044, 0.12)),
            lip: meshes.add(Capsule3d::new(0.0045, 0.03).mesh().rings(1).latitudes(6).longitudes(10)),
            ring: meshes.add(Torus::new(0.0128, 0.0152)),
            sclera: mats.add(StandardMaterial {
                base_color: Color::srgb(0.93, 0.92, 0.9),
                perceptual_roughness: 0.18,
                reflectance: 0.6,
                ..default()
            }),
            pupil: mats.add(StandardMaterial {
                base_color: Color::srgb(0.01, 0.01, 0.012),
                perceptual_roughness: 0.1,
                ..default()
            }),
            mouth: mats.add(StandardMaterial {
                base_color: Color::srgb(0.16, 0.04, 0.045),
                perceptual_roughness: 0.8,
                ..default()
            }),
            frame: mats.add(StandardMaterial {
                base_color: Color::srgb(0.04, 0.04, 0.045),
                perceptual_roughness: 0.35,
                metallic: 0.3,
                ..default()
            }),
            sole: mats.add(StandardMaterial {
                base_color: Color::srgb(0.9, 0.9, 0.88),
                perceptual_roughness: 0.8,
                ..default()
            }),
        }
    }
}

#[derive(Clone)]
pub struct Look {
    pub skin: Color,
    pub shirt: Color,
    pub pants: Color,
    pub shoes: Color,
    pub hair: Color,
    pub iris: Color,
    pub hair_style: u8, // 0 bald, 1 short, 2 long, 3 bun, 4 buzz
    pub goatee: bool,
    pub glasses: bool,
    pub female: bool,
    pub long_sleeves: bool,
    pub shorts: bool,
    pub height: f32,
}

impl Look {
    pub fn player() -> Self {
        Self {
            skin: Color::srgb(0.87, 0.68, 0.54),
            shirt: Color::srgb(0.1, 0.42, 0.52),
            pants: Color::srgb(0.12, 0.16, 0.28),
            shoes: Color::srgb(0.12, 0.12, 0.13),
            hair: Color::srgb(0.1, 0.065, 0.04),
            iris: Color::srgb(0.22, 0.12, 0.05),
            hair_style: 1,
            goatee: true,
            glasses: false,
            female: false,
            long_sleeves: true,
            shorts: false,
            height: 1.0,
        }
    }

    pub fn random(rng: &mut Rng) -> Self {
        const SKINS: [(f32, f32, f32); 6] = [
            (0.96, 0.80, 0.69),
            (0.87, 0.69, 0.55),
            (0.78, 0.57, 0.42),
            (0.63, 0.44, 0.31),
            (0.45, 0.31, 0.22),
            (0.33, 0.22, 0.16),
        ];
        const HAIRS: [(f32, f32, f32); 7] = [
            (0.05, 0.04, 0.035),
            (0.12, 0.07, 0.04),
            (0.3, 0.19, 0.1),
            (0.72, 0.58, 0.3),
            (0.42, 0.14, 0.07),
            (0.6, 0.6, 0.6),
            (0.85, 0.85, 0.85),
        ];
        const IRIS: [(f32, f32, f32); 5] = [
            (0.22, 0.12, 0.05),
            (0.2, 0.4, 0.65),
            (0.25, 0.45, 0.25),
            (0.4, 0.45, 0.5),
            (0.4, 0.3, 0.12),
        ];
        const PANTS: [(f32, f32, f32); 6] = [
            (0.12, 0.17, 0.3),
            (0.5, 0.42, 0.3),
            (0.08, 0.08, 0.09),
            (0.3, 0.3, 0.32),
            (0.15, 0.25, 0.17),
            (0.2, 0.2, 0.4),
        ];
        let s = SKINS[rng.pick(6)];
        let h = HAIRS[rng.pick(7)];
        let i = IRIS[rng.pick(5)];
        let p = PANTS[rng.pick(6)];
        let female = rng.chance(0.5);
        let hs = if female {
            [2u8, 2, 3, 1][rng.pick(4)]
        } else {
            [1u8, 1, 0, 4, 1][rng.pick(5)]
        };
        let long_sleeves = rng.chance(0.55);
        Self {
            skin: Color::srgb(s.0, s.1, s.2),
            shirt: Color::srgb(rng.range(0.1, 0.95), rng.range(0.1, 0.9), rng.range(0.1, 0.9)),
            pants: Color::srgb(p.0, p.1, p.2),
            shoes: Color::srgb(rng.range(0.05, 0.6), rng.range(0.05, 0.5), rng.range(0.05, 0.5)),
            hair: Color::srgb(h.0, h.1, h.2),
            iris: Color::srgb(i.0, i.1, i.2),
            hair_style: hs,
            goatee: !female && rng.chance(0.2),
            glasses: rng.chance(0.2),
            female,
            long_sleeves,
            shorts: !long_sleeves && rng.chance(0.4),
            height: rng.range(0.93, 1.06) * if female { 0.95 } else { 1.0 },
        }
    }
}

#[derive(Default, Clone, Copy)]
pub struct Joints {
    pub hips: Option<Entity>,
    pub spine: Option<Entity>,
    pub head: Option<Entity>,
    pub thigh: [Option<Entity>; 2], // [right(+x), left(-x)]
    pub knee: [Option<Entity>; 2],
    pub ankle: [Option<Entity>; 2],
    pub shoulder: [Option<Entity>; 2],
    pub elbow: [Option<Entity>; 2],
}

#[derive(Component)]
pub struct Humanoid {
    pub j: Joints,
    pub phase: f32,
    pub speed: f32,
    pub last: Vec3,
    pub t: f32,
    pub air: f32, // vertical offset of the body above the ground (jumping), set by the controller
    pub scale: f32,
    pub step: i32,
}

const L_THIGH: f32 = 0.44;
const L_SHIN: f32 = 0.43;
const ANKLE_H: f32 = 0.05;

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

struct Cx<'a> {
    a: &'a HumanAssets,
}

fn mat(mats: &mut Assets<StandardMaterial>, c: Color, rough: f32) -> Handle<StandardMaterial> {
    mats.add(StandardMaterial {
        base_color: c,
        perceptual_roughness: rough,
        ..default()
    })
}

fn pivot<'a>(p: &'a mut ChildSpawnerCommands<'_>, pos: Vec3) -> EntityCommands<'a> {
    p.spawn((Transform::from_translation(pos), Visibility::default()))
}

fn part(
    p: &mut ChildSpawnerCommands,
    mesh: &Handle<Mesh>,
    m: &Handle<StandardMaterial>,
    pos: Vec3,
    scale: Vec3,
    rot: Quat,
) {
    p.spawn((
        Mesh3d(mesh.clone()),
        MeshMaterial3d(m.clone()),
        Transform {
            translation: pos,
            rotation: rot,
            scale,
        },
    ));
}

/// Builds a person. Returns the root entity.
pub fn spawn_humanoid(
    commands: &mut Commands,
    assets: &HumanAssets,
    mats: &mut Assets<StandardMaterial>,
    look: &Look,
    mut tf: Transform,
) -> Entity {
    let cx = Cx { a: assets };
    let a = cx.a;
    let skin = mat(mats, look.skin, 0.55);
    let lin = look.skin.to_linear();
    let lip_c = if look.female {
        Color::linear_rgb(lin.red * 1.1 + 0.08, lin.green * 0.55, lin.blue * 0.6)
    } else {
        Color::linear_rgb(lin.red * 0.92, lin.green * 0.62, lin.blue * 0.62)
    };
    let lip = mat(mats, lip_c, 0.4);
    let ear = mat(
        mats,
        Color::linear_rgb(lin.red * 0.92, lin.green * 0.8, lin.blue * 0.78),
        0.6,
    );
    let shirt = mat(mats, look.shirt, 0.85);
    let pants = mat(mats, look.pants, 0.85);
    let shoe = mat(mats, look.shoes, 0.6);
    let hair = mat(mats, look.hair, 0.5);
    let iris = mat(mats, look.iris, 0.25);
    let belt = mat(mats, Color::srgb(0.06, 0.05, 0.04), 0.5);

    tf.scale = Vec3::splat(look.height);
    let mut j = Joints::default();
    let shoulder_x = if look.female { 0.178 } else { 0.2 };
    let chest_scale = if look.female {
        Vec3::new(1.0, 1.0, 0.72)
    } else {
        Vec3::new(1.05, 1.0, 0.72)
    };

    let root = commands
        .spawn((tf, Visibility::default()))
        .with_children(|r| {
            j.hips = pivot(r, Vec3::new(0.0, L_THIGH + L_SHIN + ANKLE_H + 0.02, 0.0))
                .with_children(|h| {
                    // pelvis + belt
                    part(
                        h,
                        &a.pelvis,
                        &pants,
                        Vec3::new(0.0, 0.0, 0.0),
                        Vec3::new(1.0, 0.95, if look.female { 0.85 } else { 0.8 }),
                        Quat::from_rotation_z(FRAC_PI_2),
                    );
                    part(
                        h,
                        &a.cube,
                        &belt,
                        Vec3::new(0.0, 0.085, 0.0),
                        Vec3::new(0.335, 0.03, 0.2),
                        Quat::IDENTITY,
                    );

                    // legs
                    for (si, side) in [1.0f32, -1.0].into_iter().enumerate() {
                        let shin_mat = if look.shorts { &skin } else { &pants };
                        j.thigh[si] = pivot(h, Vec3::new(side * 0.092, -0.02, 0.0))
                            .with_children(|t| {
                                part(
                                    t,
                                    &a.thigh,
                                    &pants,
                                    Vec3::new(0.0, -0.215, 0.0),
                                    Vec3::ONE,
                                    Quat::IDENTITY,
                                );
                                j.knee[si] = pivot(t, Vec3::new(0.0, -L_THIGH, 0.0))
                                    .with_children(|k| {
                                        part(
                                            k,
                                            &a.sphere,
                                            &pants,
                                            Vec3::ZERO,
                                            Vec3::splat(0.062),
                                            Quat::IDENTITY,
                                        );
                                        part(
                                            k,
                                            &a.shin,
                                            shin_mat,
                                            Vec3::new(0.0, -0.215, 0.0),
                                            Vec3::ONE,
                                            Quat::IDENTITY,
                                        );
                                        j.ankle[si] = pivot(k, Vec3::new(0.0, -L_SHIN, 0.0))
                                            .with_children(|f| {
                                                part(
                                                    f,
                                                    &a.shoe,
                                                    &shoe,
                                                    Vec3::new(0.0, -0.02, -0.065),
                                                    Vec3::new(1.0, 0.8, 1.0),
                                                    Quat::from_rotation_x(FRAC_PI_2),
                                                );
                                                part(
                                                    f,
                                                    &a.shoe,
                                                    &a.sole,
                                                    Vec3::new(0.0, -0.043, -0.065),
                                                    Vec3::new(1.02, 1.03, 0.42),
                                                    Quat::from_rotation_x(FRAC_PI_2),
                                                );
                                            })
                                            .id()
                                            .into();
                                    })
                                    .id()
                                    .into();
                            })
                            .id()
                            .into();
                    }

                    // spine / upper body
                    j.spine = pivot(h, Vec3::new(0.0, 0.06, 0.0))
                        .with_children(|s| {
                            part(
                                s,
                                &a.chest,
                                &shirt,
                                Vec3::new(0.0, 0.285, 0.0),
                                chest_scale * Vec3::new(1.12, 0.85, 1.0),
                                Quat::IDENTITY,
                            );
                            part(
                                s,
                                &a.pelvis,
                                &shirt,
                                Vec3::new(0.0, 0.06, 0.0),
                                Vec3::new(1.12, 0.8, 0.95),
                                Quat::from_rotation_z(FRAC_PI_2),
                            );
                            // neck
                            part(
                                s,
                                &a.neck,
                                &skin,
                                Vec3::new(0.0, 0.52, 0.0),
                                Vec3::ONE,
                                Quat::IDENTITY,
                            );
                            // shoulders & arms
                            for (si, side) in [1.0f32, -1.0].into_iter().enumerate() {
                                let fore_mat = if look.long_sleeves { &shirt } else { &skin };
                                j.shoulder[si] = pivot(s, Vec3::new(side * shoulder_x, 0.43, 0.0))
                                    .with_children(|sh| {
                                        part(
                                            sh,
                                            &a.sphere,
                                            &shirt,
                                            Vec3::ZERO,
                                            Vec3::splat(0.05),
                                            Quat::IDENTITY,
                                        );
                                        part(
                                            sh,
                                            &a.upper_arm,
                                            &shirt,
                                            Vec3::new(0.0, -0.14, 0.0),
                                            Vec3::ONE,
                                            Quat::IDENTITY,
                                        );
                                        j.elbow[si] = pivot(sh, Vec3::new(0.0, -0.29, 0.0))
                                            .with_children(|e| {
                                                part(
                                                    e,
                                                    &a.sphere,
                                                    &shirt,
                                                    Vec3::ZERO,
                                                    Vec3::splat(0.045),
                                                    Quat::IDENTITY,
                                                );
                                                part(
                                                    e,
                                                    &a.forearm,
                                                    fore_mat,
                                                    Vec3::new(0.0, -0.125, 0.0),
                                                    Vec3::ONE,
                                                    Quat::IDENTITY,
                                                );
                                                // hand
                                                part(
                                                    e,
                                                    &a.sphere,
                                                    &skin,
                                                    Vec3::new(0.0, -0.285, 0.0),
                                                    Vec3::new(0.034, 0.05, 0.022),
                                                    Quat::IDENTITY,
                                                );
                                                // thumb
                                                part(
                                                    e,
                                                    &a.sphere,
                                                    &skin,
                                                    Vec3::new(-side * 0.03, -0.265, -0.012),
                                                    Vec3::new(0.011, 0.028, 0.011),
                                                    Quat::from_rotation_z(side * 0.2),
                                                );
                                            })
                                            .id()
                                            .into();
                                    })
                                    .id()
                                    .into();
                            }

                            // head
                            j.head = pivot(s, Vec3::new(0.0, 0.57, 0.0))
                                .with_children(|hd| {
                                    build_head(hd, a, look, &skin, &lip, &ear, &hair, &iris);
                                })
                                .id()
                                .into();
                        })
                        .id()
                        .into();
                })
                .id()
                .into();
        })
        .insert(Humanoid {
            j,
            phase: 0.0,
            speed: 0.0,
            last: tf.translation,
            t: (tf.translation.x * 7.3 + tf.translation.z * 3.1).abs() % 20.0,
            air: 0.0,
            scale: look.height,
            step: 0,
        })
        .id();
    root
}

#[allow(clippy::too_many_arguments)]
fn build_head(
    hd: &mut ChildSpawnerCommands,
    a: &HumanAssets,
    look: &Look,
    skin: &Handle<StandardMaterial>,
    lip: &Handle<StandardMaterial>,
    ear: &Handle<StandardMaterial>,
    hair: &Handle<StandardMaterial>,
    iris: &Handle<StandardMaterial>,
) {
    // Everything is positioned relative to the skull centre (0.07 above the neck top).
    // `fz(x, y)` returns the z of the face surface so features sit exactly on it.
    let female = look.female;
    let skull_r = (0.082f32, 0.102f32, 0.095f32);
    let jaw_c = (0.0f32, -0.066f32, -0.022f32);
    let jaw_r = if female {
        (0.06f32, 0.058f32, 0.07f32)
    } else {
        (0.066f32, 0.062f32, 0.075f32)
    };
    let fz = move |x: f32, y: f32| -> f32 {
        let e = |cx: f32, cy: f32, cz: f32, r: (f32, f32, f32)| {
            let u = (x - cx) / r.0;
            let w = (y - cy) / r.1;
            let k = 1.0 - u * u - w * w;
            if k > 0.0 {
                cz - r.2 * k.sqrt()
            } else {
                f32::MAX
            }
        };
        e(0.0, 0.0, 0.0, skull_r).min(e(jaw_c.0, jaw_c.1, jaw_c.2, jaw_r))
    };

    hd.spawn((Transform::from_xyz(0.0, 0.07, 0.0), Visibility::default()))
        .with_children(|f| {
            let q = Quat::IDENTITY;
            let v = Vec3::new;
            part(f, &a.sphere, skin, v(0.0, 0.0, 0.0), v(skull_r.0, skull_r.1, skull_r.2), q);
            part(f, &a.sphere, skin, v(jaw_c.0, jaw_c.1, jaw_c.2), v(jaw_r.0, jaw_r.1, jaw_r.2), q);
            // brow ridge, subtle
            part(f, &a.sphere, skin, v(0.0, 0.028, fz(0.0, 0.028) + 0.006), v(0.05, 0.012, 0.011), q);

            for s in [1.0f32, -1.0] {
                // eyes: socket shadow, eyeball, iris, pupil, lids, lashes
                let (ex, ey) = (s * 0.031, 0.0f32);
                let z = fz(ex, ey);
                part(f, &a.sphere, ear, v(ex, ey, z + 0.0105), v(0.0205, 0.0175, 0.0105), q);
                part(f, &a.sphere, &a.sclera, v(ex, ey, z + 0.0085), v(0.0145, 0.0128, 0.0098), q);
                part(f, &a.sphere, iris, v(ex, ey, z + 0.0003), v(0.0088, 0.0088, 0.0042), q);
                part(f, &a.sphere, &a.pupil, v(ex, ey, z - 0.0028), v(0.0041, 0.0041, 0.002), q);
                let yu = ey + 0.0112;
                part(f, &a.sphere, skin, v(ex, yu, fz(ex, yu) + 0.0073), v(0.0155, 0.0072, 0.0108), q);
                let yl = ey - 0.0105;
                part(f, &a.sphere, skin, v(ex, yl, fz(ex, yl) + 0.007), v(0.0148, 0.0046, 0.0097), q);
                let lash_h = if female { 0.0035 } else { 0.0022 };
                let yla = ey + 0.0152;
                part(
                    f,
                    &a.cube,
                    hair,
                    v(ex, yla, fz(ex, yla) + 0.0008),
                    v(0.029, lash_h, 0.002),
                    Quat::from_rotation_z(-s * 0.05),
                );
                // brows
                let brow_h = if female { 0.0042 } else { 0.0068 };
                let (bx, by) = (s * 0.0335, 0.0305);
                part(
                    f,
                    &a.cube,
                    hair,
                    v(bx, by, fz(bx, by) + 0.0008),
                    v(0.032, brow_h, 0.0075),
                    Quat::from_rotation_y(-s * 0.5) * Quat::from_rotation_z(s * 0.14),
                );
                // ears
                part(f, &a.sphere, ear, v(s * 0.0815, -0.022, 0.006), v(0.0085, 0.026, 0.0185), q);
                // cheeks
                let (cx, cy) = (s * 0.043, -0.04);
                part(f, &a.sphere, skin, v(cx, cy, fz(cx, cy) + 0.0085), v(0.022, 0.02, 0.0095), q);
                // nostril wings and nostrils
                let (nx, ny) = (s * 0.0112, -0.043);
                part(f, &a.sphere, skin, v(nx, ny, fz(nx, ny) + 0.0005), v(0.0095, 0.0082, 0.0092), q);
                let (nx2, ny2) = (s * 0.0072, -0.0485);
                part(f, &a.sphere, &a.mouth, v(nx2, ny2, fz(nx2, ny2) - 0.0078), v(0.0034, 0.0022, 0.0034), q);
            }

            // nose bridge + tip
            part(
                f,
                &a.cube,
                skin,
                v(0.0, -0.014, fz(0.0, -0.014) - 0.001),
                v(0.0145, 0.046, 0.012),
                Quat::from_rotation_x(0.22),
            );
            part(f, &a.sphere, skin, v(0.0, -0.04, fz(0.0, -0.04) - 0.0052), v(0.0112, 0.0102, 0.0108), q);

            // mouth
            let ym = -0.07;
            part(f, &a.cube, &a.mouth, v(0.0, ym, fz(0.0, ym) - 0.0004), v(0.03, 0.0022, 0.004), q);
            part(
                f,
                &a.lip,
                lip,
                v(0.0, ym + 0.0035, fz(0.0, ym + 0.0035) + 0.0005),
                v(1.0, 0.85, 1.0),
                Quat::from_rotation_z(FRAC_PI_2),
            );
            part(
                f,
                &a.lip,
                lip,
                v(0.0, ym - 0.0075, fz(0.0, ym - 0.0075) + 0.0005),
                v(1.0, 0.95, 1.0),
                Quat::from_rotation_z(FRAC_PI_2),
            );

            // hair
            match look.hair_style {
                0 => {}
                4 => {
                    part(f, &a.sphere, hair, v(0.0, 0.016, 0.008), v(0.0845, 0.1055, 0.0975), q);
                }
                s => {
                    part(
                        f,
                        &a.sphere,
                        hair,
                        v(0.0, 0.032, 0.015),
                        v(0.0895, 0.1015, 0.1015),
                        Quat::from_rotation_x(0.12),
                    );
                    if s == 2 {
                        part(f, &a.sphere, hair, v(0.0, -0.07, 0.06), v(0.082, 0.135, 0.05), q);
                        for sd in [1.0f32, -1.0] {
                            part(f, &a.sphere, hair, v(sd * 0.082, -0.05, 0.015), v(0.015, 0.1, 0.045), q);
                        }
                    }
                    if s == 3 {
                        part(f, &a.sphere, hair, v(0.0, 0.105, 0.07), v(0.04, 0.04, 0.04), q);
                    }
                }
            }

            if look.goatee {
                part(f, &a.sphere, hair, v(0.0, -0.108, fz(0.0, -0.108) + 0.003), v(0.0215, 0.022, 0.009), q);
                part(f, &a.cube, hair, v(0.0, -0.0585, fz(0.0, -0.0585) - 0.002), v(0.03, 0.0065, 0.0075), q);
            }

            if look.glasses {
                let zr = fz(0.031, 0.0) - 0.007;
                for s in [1.0f32, -1.0] {
                    part(f, &a.ring, &a.frame, v(s * 0.031, 0.0, zr), Vec3::ONE, Quat::from_rotation_x(FRAC_PI_2));
                    let (x0, z0, x1, z1) = (0.0465f32, zr, 0.081f32, 0.005f32);
                    let (dx, dz) = (x1 - x0, z1 - z0);
                    part(
                        f,
                        &a.cube,
                        &a.frame,
                        v(s * (x0 + x1) / 2.0, 0.004, (z0 + z1) / 2.0),
                        v(0.003, 0.003, (dx * dx + dz * dz).sqrt()),
                        Quat::from_rotation_y(s * dx.atan2(dz)),
                    );
                }
                part(f, &a.cube, &a.frame, v(0.0, 0.004, zr - 0.001), v(0.016, 0.003, 0.003), q);
            }
        });
}

/// Updates every humanoid's joints from how fast its root actually moved this frame.
pub fn animate_humanoids(
    time: Res<Time>,
    mut hs: Query<(&mut Humanoid, &Transform)>,
    mut tfs: Query<&mut Transform, Without<Humanoid>>,
) {
    let dt = time.delta_secs().max(1e-4);
    for (mut h, root) in &mut hs {
        let pos = root.translation;
        let d = Vec2::new(pos.x - h.last.x, pos.z - h.last.z).length() / h.scale.max(0.1);
        let inst = if d > 2.0 { 0.0 } else { d / dt };
        h.last = pos;
        let k = smooth(10.0, dt);
        h.speed += (inst - h.speed) * k;
        h.t += dt;
        let sp = h.speed;

        let sw = smoothstep(0.05, 0.7, sp); // walking amount
        let run = smoothstep(3.0, 5.2, sp);
        let cycle = lerp(1.55 + 0.2 * sp, 2.1 + 0.33 * sp, run);
        h.phase = (h.phase + TAU * sp * dt / cycle) % (TAU * 100.0);
        let ph = h.phase;
        let amp = (cycle / lerp(3.7, 5.1, run)).min(0.95) * sw;

        let mut theta = [0.0f32; 2];
        let mut kappa = [0.0f32; 2];
        for i in 0..2 {
            let p = ph + if i == 0 { 0.0 } else { PI };
            theta[i] = amp * p.sin() + 0.04 * sw;
            let swing = (p + 0.1).cos().max(0.0).powf(1.4);
            kappa[i] = (0.05 + (0.85 + 0.65 * run) * swing) * sw + 0.03 * run;
        }

        // hip height from the lowest foot so stance feet stay planted
        let extent = |t: f32, k: f32| L_THIGH * t.cos() + L_SHIN * (t - k).cos();
        let ext = extent(theta[0], kappa[0]).max(extent(theta[1], kappa[1]));
        let flight = run * 0.055 * (2.0 * ph).sin().abs().powf(0.6);
        let hips_y = ANKLE_H + 0.02 + ext + flight + h.air;

        let idle_w = 1.0 - sw;
        let t = h.t;
        let breathe = (t * 1.7).sin();

        if let Some(e) = h.j.hips {
            if let Ok(mut tf) = tfs.get_mut(e) {
                tf.translation.y = hips_y;
                tf.translation.x = 0.012 * (ph).sin() * sw + 0.006 * (t * 0.5).sin() * idle_w;
                tf.rotation = Quat::from_rotation_y(0.09 * ph.sin() * sw * (1.0 + run))
                    * Quat::from_rotation_z(-0.035 * ph.sin() * sw + 0.012 * (t * 0.5).sin() * idle_w);
            }
        }
        if let Some(e) = h.j.spine {
            if let Ok(mut tf) = tfs.get_mut(e) {
                let lean = (0.02 + 0.06 * sw + 0.2 * run) * -1.0;
                tf.rotation = Quat::from_rotation_y(-0.12 * ph.sin() * sw * (1.0 + run))
                    * Quat::from_rotation_z(0.03 * ph.sin() * sw)
                    * Quat::from_rotation_x(lean + 0.012 * breathe * idle_w);
            }
        }
        if let Some(e) = h.j.head {
            if let Ok(mut tf) = tfs.get_mut(e) {
                let look = 0.28 * (t * 0.37).sin() * (t * 0.11).cos() * idle_w;
                let nod = 0.04 * (t * 0.23).sin() * idle_w;
                tf.rotation = Quat::from_rotation_y(look + 0.07 * ph.sin() * sw)
                    * Quat::from_rotation_x(nod + 0.05 * run + 0.02 * (2.0 * ph).cos().abs() * sw);
            }
        }
        for i in 0..2 {
            let side = if i == 0 { 1.0 } else { -1.0 };
            let p = ph + if i == 0 { 0.0 } else { PI };
            if let Some(e) = h.j.thigh[i] {
                if let Ok(mut tf) = tfs.get_mut(e) {
                    tf.rotation = Quat::from_rotation_z(side * 0.02) * Quat::from_rotation_x(theta[i]);
                }
            }
            if let Some(e) = h.j.knee[i] {
                if let Ok(mut tf) = tfs.get_mut(e) {
                    tf.rotation = Quat::from_rotation_x(-kappa[i]);
                }
            }
            if let Some(e) = h.j.ankle[i] {
                if let Ok(mut tf) = tfs.get_mut(e) {
                    let stance = smoothstep(-0.2, 0.6, -p.cos());
                    let shank = theta[i] - kappa[i];
                    tf.rotation = Quat::from_rotation_x(-shank * stance + 0.1 * (1.0 - stance) * sw);
                }
            }
            // arms swing against the legs
            let arm_amp = lerp(0.42, 1.0, run) * sw * (amp / 0.55).clamp(0.3, 1.2);
            let arm = -arm_amp * p.sin() * 0.85;
            if let Some(e) = h.j.shoulder[i] {
                if let Ok(mut tf) = tfs.get_mut(e) {
                    tf.rotation = Quat::from_rotation_z(side * (0.07 + 0.03 * breathe * idle_w + 0.1 * run))
                        * Quat::from_rotation_x(arm);
                }
            }
            if let Some(e) = h.j.elbow[i] {
                if let Ok(mut tf) = tfs.get_mut(e) {
                    let flex = 0.18 + 0.12 * sw + (0.3 + 1.0 * run) * (-p.sin()).max(0.0) * sw + 1.0 * run;
                    tf.rotation = Quat::from_rotation_x(flex.min(2.0));
                }
            }
        }
    }
}
