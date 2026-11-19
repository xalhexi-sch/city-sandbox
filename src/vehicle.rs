//! Cars: detailed hollow-cabin model, spinning/steering wheels, slip-based physics with
//! weight transfer, drifting, collisions, and a traffic AI that obeys signals.

use crate::common::*;
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, TAU};

pub const WHEEL_R: f32 = 0.33;
pub const WHEELBASE: f32 = 2.7;
const AXLE_F: f32 = -1.38;
const AXLE_R: f32 = 1.32;
const TRACK: f32 = 0.8;
const BODY_PIVOT: f32 = 0.45;
pub const DRIVER_EYE: Vec3 = Vec3::new(-0.4, 1.25, 0.2);

// ------------------------------------------------------------ components ---
#[derive(Component)]
pub struct Vehicle {
    pub body: Entity,
    pub pivots: [Entity; 4], // FL, FR, RL, RR
    pub spins: [Entity; 4],
    pub steering: Entity,
    pub driver: Entity,
    pub tail_mat: Handle<StandardMaterial>,
    pub speed: f32, // signed, along heading
    pub steer: f32,
    pub spin: f32,
    pub pitch: f32,
    pub roll: f32,
    pub braking: bool,
    pub shown_braking: bool,
    pub lock_rear: bool,
}

#[derive(Component, Default)]
pub struct Car {
    pub vel: Vec2,
    pub yaw_rate: f32,
    pub acc_long: f32,
    pub pitch_v: f32,
    pub roll_v: f32,
    pub slip: f32,
    pub shake: f32,
    pub skid_t: f32,
    pub input_throttle: f32,
}

#[derive(Component)]
pub struct Traffic {
    pub dir: Vec3,
    pub desired: f32,
    pub stuck: f32,
    pub hit: f32,
    pub acc: f32,
}

#[derive(Component)]
pub struct StartCar;

#[derive(Component)]
pub struct Skid {
    pub life: f32,
}

#[derive(Resource)]
pub struct CarAssets {
    pub cube: Handle<Mesh>,
    pub rbox: Handle<Mesh>,
    pub sphere: Handle<Mesh>,
    pub tire: Handle<Mesh>,
    pub rim: Handle<Mesh>,
    pub hub: Handle<Mesh>,
    pub wheel_ring: Handle<Mesh>,
    pub tire_m: Handle<StandardMaterial>,
    pub rim_m: Handle<StandardMaterial>,
    pub dark: Handle<StandardMaterial>,
    pub glass: Handle<StandardMaterial>,
    pub seat: Handle<StandardMaterial>,
    pub chrome: Handle<StandardMaterial>,
    pub plate: Handle<StandardMaterial>,
    pub headlight: Handle<StandardMaterial>,
    pub interior: Handle<StandardMaterial>,
    pub skid_mesh: Handle<Mesh>,
    pub skid_m: Handle<StandardMaterial>,
    pub paints: Vec<Handle<StandardMaterial>>,
}

impl CarAssets {
    pub fn new(meshes: &mut Assets<Mesh>, mats: &mut Assets<StandardMaterial>) -> Self {
        let m = |mats: &mut Assets<StandardMaterial>, c: Color, r: f32, me: f32| {
            mats.add(StandardMaterial {
                base_color: c,
                perceptual_roughness: r,
                metallic: me,
                ..default()
            })
        };
        let colors: [(f32, f32, f32); 10] = [
            (0.75, 0.06, 0.06),
            (0.08, 0.22, 0.65),
            (0.9, 0.9, 0.9),
            (0.06, 0.06, 0.07),
            (0.92, 0.72, 0.1),
            (0.08, 0.45, 0.25),
            (0.5, 0.52, 0.56),
            (0.8, 0.38, 0.08),
            (0.35, 0.05, 0.12),
            (0.65, 0.68, 0.72),
        ];
        let paints = colors
            .iter()
            .map(|&(r, g, b)| {
                mats.add(StandardMaterial {
                    base_color: Color::srgb(r, g, b),
                    perceptual_roughness: 0.32,
                    metallic: 0.55,
                    reflectance: 0.6,
                    clearcoat: 1.0,
                    clearcoat_perceptual_roughness: 0.08,
                    ..default()
                })
            })
            .collect();
        Self {
            cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
            rbox: meshes.add(rounded_box(0.42, 12)),
            sphere: meshes.add(Sphere::new(1.0).mesh().uv(20, 12)),
            tire: meshes.add(Cylinder::new(WHEEL_R, 0.22).mesh().resolution(28)),
            rim: meshes.add(Cylinder::new(0.215, 0.23).mesh().resolution(20)),
            hub: meshes.add(Cylinder::new(0.05, 0.03).mesh().resolution(12)),
            wheel_ring: meshes.add(Torus::new(0.17, 0.195)),
            tire_m: m(mats, Color::srgb(0.025, 0.025, 0.028), 0.92, 0.0),
            rim_m: m(mats, Color::srgb(0.72, 0.73, 0.76), 0.28, 0.95),
            dark: m(mats, Color::srgb(0.03, 0.03, 0.035), 0.55, 0.2),
            glass: mats.add(StandardMaterial {
                base_color: Color::srgba(0.1, 0.15, 0.19, 0.42),
                perceptual_roughness: 0.04,
                reflectance: 0.7,
                alpha_mode: AlphaMode::Blend,
                ..default()
            }),
            seat: m(mats, Color::srgb(0.1, 0.1, 0.115), 0.9, 0.0),
            chrome: m(mats, Color::srgb(0.85, 0.85, 0.88), 0.15, 1.0),
            plate: m(mats, Color::srgb(0.9, 0.9, 0.85), 0.6, 0.0),
            headlight: mats.add(StandardMaterial {
                base_color: Color::srgb(0.9, 0.92, 1.0),
                emissive: LinearRgba::rgb(0.3, 0.3, 0.3),
                perceptual_roughness: 0.1,
                ..default()
            }),
            interior: mats.add(StandardMaterial {
                base_color: Color::srgb(0.2, 0.19, 0.18),
                emissive: LinearRgba::rgb(0.09, 0.085, 0.08),
                perceptual_roughness: 0.9,
                ..default()
            }),
            skid_mesh: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
            skid_m: mats.add(StandardMaterial {
                base_color: Color::srgba(0.0, 0.0, 0.0, 0.8),
                alpha_mode: AlphaMode::Blend,
                perceptual_roughness: 1.0,
                ..default()
            }),
            paints,
        }
    }
}


/// Unit rounded box (extent -0.5..0.5) with smooth normals. `r` is the bevel radius in
/// unit-cube space (0..1); it scales anisotropically with the Transform, giving soft car panels.
pub fn rounded_box(r: f32, seg: usize) -> Mesh {
    use bevy::render::mesh::{Indices, PrimitiveTopology};
    use bevy::render::render_asset::RenderAssetUsages;
    let mut pos: Vec<[f32; 3]> = Vec::new();
    let mut nor: Vec<[f32; 3]> = Vec::new();
    let mut uv: Vec<[f32; 2]> = Vec::new();
    let mut idx: Vec<u32> = Vec::new();
    let lim = 1.0 - r;
    for face in 0..6 {
        let (axis, sign) = (face / 2, if face % 2 == 0 { 1.0f32 } else { -1.0 });
        let (u_ax, v_ax) = match axis {
            0 => (2, 1),
            1 => (0, 2),
            _ => (0, 1),
        };
        let base = pos.len() as u32;
        for j in 0..=seg {
            for i in 0..=seg {
                let mut p = [0.0f32; 3];
                p[axis] = sign;
                p[u_ax] = -1.0 + 2.0 * i as f32 / seg as f32;
                p[v_ax] = -1.0 + 2.0 * j as f32 / seg as f32;
                let q = [p[0].clamp(-lim, lim), p[1].clamp(-lim, lim), p[2].clamp(-lim, lim)];
                let d = Vec3::new(p[0] - q[0], p[1] - q[1], p[2] - q[2]);
                let n = d.normalize_or_zero();
                let out = Vec3::new(q[0], q[1], q[2]) + n * r;
                pos.push([out.x * 0.5, out.y * 0.5, out.z * 0.5]);
                nor.push(n.to_array());
                uv.push([i as f32 / seg as f32, j as f32 / seg as f32]);
            }
        }
        let w = (seg + 1) as u32;
        for j in 0..seg as u32 {
            for i in 0..seg as u32 {
                let a = base + j * w + i;
                let (b, c, d) = (a + 1, a + w, a + w + 1);
                // winding depends on face orientation so triangles face outward
                let flip = if axis == 2 { sign < 0.0 } else { sign > 0.0 };
                if flip {
                    idx.extend_from_slice(&[a, c, b, b, c, d]);
                } else {
                    idx.extend_from_slice(&[a, b, c, b, d, c]);
                }
            }
        }
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nor)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
        .with_inserted_indices(Indices::U32(idx))
}

fn bxr(
    p: &mut ChildSpawnerCommands<'_>,
    a: &CarAssets,
    m: &Handle<StandardMaterial>,
    c: Vec3,
    s: Vec3,
    r: Quat,
) {
    p.spawn((
        Mesh3d(a.rbox.clone()),
        MeshMaterial3d(m.clone()),
        Transform {
            translation: c,
            rotation: r,
            scale: s,
        },
    ));
}

fn bx(
    p: &mut ChildSpawnerCommands<'_>,
    a: &CarAssets,
    m: &Handle<StandardMaterial>,
    c: Vec3,
    s: Vec3,
    r: Quat,
) {
    p.spawn((
        Mesh3d(a.cube.clone()),
        MeshMaterial3d(m.clone()),
        Transform {
            translation: c,
            rotation: r,
            scale: s,
        },
    ));
}

fn bar(
    p: &mut ChildSpawnerCommands<'_>,
    a: &CarAssets,
    m: &Handle<StandardMaterial>,
    from: Vec3,
    to: Vec3,
    t: f32,
) {
    let d = to - from;
    let len = d.length();
    let up = if (d / len).y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
    let tf = Transform::from_translation((from + to) / 2.0).looking_to(d / len, up);
    p.spawn((
        Mesh3d(a.cube.clone()),
        MeshMaterial3d(m.clone()),
        Transform {
            translation: tf.translation,
            rotation: tf.rotation,
            scale: Vec3::new(t, t, len),
        },
    ));
}

/// Builds a full car. `driver_skin/shirt` give the (hidden-when-player-drives) occupant colours.
pub fn spawn_vehicle(
    commands: &mut Commands,
    a: &CarAssets,
    mats: &mut Assets<StandardMaterial>,
    paint: &Handle<StandardMaterial>,
    tf: Transform,
    driver_look: (Color, Color),
) -> Entity {
    let tail_mat = mats.add(StandardMaterial {
        base_color: Color::srgb(0.5, 0.02, 0.02),
        emissive: LinearRgba::rgb(0.5, 0.02, 0.02),
        perceptual_roughness: 0.2,
        ..default()
    });
    let driver_skin = mats.add(StandardMaterial {
        base_color: driver_look.0,
        perceptual_roughness: 0.6,
        ..default()
    });
    let driver_shirt = mats.add(StandardMaterial {
        base_color: driver_look.1,
        perceptual_roughness: 0.85,
        ..default()
    });

    let mut body = Entity::PLACEHOLDER;
    let mut pivots = [Entity::PLACEHOLDER; 4];
    let mut spins = [Entity::PLACEHOLDER; 4];
    let mut steering = Entity::PLACEHOLDER;
    let mut driver = Entity::PLACEHOLDER;
    let b = |x: f32, y: f32, z: f32| Vec3::new(x, y - BODY_PIVOT, z);
    let q0 = Quat::IDENTITY;

    let root = commands
        .spawn((tf, Visibility::default()))
        .with_children(|r| {
            body = r
                .spawn((
                    Transform::from_xyz(0.0, BODY_PIVOT, 0.0),
                    Visibility::default(),
                ))
                .with_children(|p| {
                    // underbody + floor
                    bx(p, a, &a.dark, b(0.0, 0.3, 0.0), Vec3::new(1.78, 0.1, 4.3), q0);
                    bx(p, a, &a.interior, b(0.0, 0.36, 0.1), Vec3::new(1.64, 0.02, 2.3), q0);
                    // hood, nose and front fascia
                    bxr(p, a, paint, b(0.0, 0.66, -1.62), Vec3::new(1.78, 0.26, 1.36), Quat::from_rotation_x(-0.045));
                    bxr(p, a, paint, b(0.0, 0.52, -2.2), Vec3::new(1.8, 0.44, 0.22), q0);
                    bxr(p, a, &a.dark, b(0.0, 0.43, -2.31), Vec3::new(1.82, 0.16, 0.1), q0); // bumper lip
                    bx(p, a, &a.dark, b(0.0, 0.6, -2.315), Vec3::new(0.7, 0.14, 0.03), q0); // grille
                    for s in [1.0f32, -1.0] {
                        bx(p, a, &a.headlight, b(s * 0.62, 0.64, -2.31), Vec3::new(0.34, 0.13, 0.03), q0);
                    }
                    bx(p, a, &a.plate, b(0.0, 0.46, -2.37), Vec3::new(0.5, 0.11, 0.01), q0);
                    // trunk, tail and rear fascia
                    bxr(p, a, paint, b(0.0, 0.7, 1.76), Vec3::new(1.78, 0.3, 0.96), Quat::from_rotation_x(0.03));
                    bxr(p, a, paint, b(0.0, 0.52, 2.17), Vec3::new(1.8, 0.44, 0.2), q0);
                    bxr(p, a, &a.dark, b(0.0, 0.43, 2.28), Vec3::new(1.82, 0.16, 0.1), q0);
                    for s in [1.0f32, -1.0] {
                        bx(p, a, &tail_mat, b(s * 0.66, 0.66, 2.28), Vec3::new(0.4, 0.12, 0.03), q0);
                    }
                    bx(p, a, &a.plate, b(0.0, 0.5, 2.3), Vec3::new(0.5, 0.11, 0.01), q0);
                    // doors / sides
                    for s in [1.0f32, -1.0] {
                        bxr(p, a, paint, b(s * 0.89, 0.62, 0.15), Vec3::new(0.08, 0.5, 2.35), q0);
                        bx(p, a, &a.dark, b(s * 0.915, 0.4, 0.15), Vec3::new(0.05, 0.06, 2.1), q0); // sill
                        // fenders over the wheels
                        for z in [AXLE_F, AXLE_R] {
                            bxr(p, a, paint, b(s * 0.84, 0.72, z), Vec3::new(0.2, 0.2, 1.05), q0);
                        }
                        // door handle + mirror
                        bx(p, a, &a.chrome, b(s * 0.935, 0.78, 0.05), Vec3::new(0.02, 0.03, 0.16), q0);
                        bxr(p, a, paint, b(s * 1.0, 1.0, -0.78), Vec3::new(0.12, 0.09, 0.07), q0);
                    }
                    // roof, pillars
                    bxr(p, a, paint, b(0.0, 1.58, 0.18), Vec3::new(1.7, 0.06, 1.62), q0);
                    bx(p, a, &a.interior, b(0.0, 1.545, 0.18), Vec3::new(1.6, 0.01, 1.5), q0);
                    for s in [1.0f32, -1.0] {
                        bar(p, a, paint, b(s * 0.86, 0.87, -0.98), b(s * 0.8, 1.58, -0.5), 0.07);
                        bar(p, a, paint, b(s * 0.89, 0.87, 0.22), b(s * 0.85, 1.58, 0.22), 0.07);
                        bar(p, a, paint, b(s * 0.86, 0.87, 1.3), b(s * 0.8, 1.58, 0.95), 0.08);
                    }
                    // glass
                    {
                        let c = b(0.0, 1.225, -0.74);
                        let d = Vec3::new(0.0, 0.71, 0.48);
                        let t = Transform::from_translation(c).looking_to(d.normalize(), Vec3::Y);
                        p.spawn((
                            Mesh3d(a.cube.clone()),
                            MeshMaterial3d(a.glass.clone()),
                            Transform {
                                translation: t.translation,
                                rotation: t.rotation,
                                scale: Vec3::new(1.6, 0.01, d.length()),
                            },
                        ));
                        let c = b(0.0, 1.225, 1.125);
                        let d = Vec3::new(0.0, 0.71, -0.35);
                        let t = Transform::from_translation(c).looking_to(d.normalize(), Vec3::Y);
                        p.spawn((
                            Mesh3d(a.cube.clone()),
                            MeshMaterial3d(a.glass.clone()),
                            Transform {
                                translation: t.translation,
                                rotation: t.rotation,
                                scale: Vec3::new(1.55, 0.01, d.length()),
                            },
                        ));
                    }
                    for s in [1.0f32, -1.0] {
                        bx(p, a, &a.glass, b(s * 0.885, 1.235, -0.38), Vec3::new(0.01, 0.63, 0.95), q0);
                        bx(p, a, &a.glass, b(s * 0.885, 1.235, 0.72), Vec3::new(0.01, 0.63, 0.9), q0);
                    }
                    // dashboard & cluster
                    bxr(p, a, &a.interior, b(0.0, 0.84, -0.82), Vec3::new(1.7, 0.18, 0.36), q0);
                    bx(p, a, &a.dark, b(0.0, 0.955, -0.74), Vec3::new(1.6, 0.05, 0.26), Quat::from_rotation_x(0.2));
                    bx(p, a, &a.dark, b(-0.4, 0.96, -0.66), Vec3::new(0.34, 0.12, 0.06), q0);
                    bx(p, a, &a.dark, b(0.0, 0.84, -0.62), Vec3::new(0.3, 0.2, 0.06), q0); // centre stack
                    // mirror
                    bx(p, a, &a.dark, b(0.0, 1.45, -0.52), Vec3::new(0.2, 0.05, 0.025), q0);
                    // steering column + wheel
                    bar(p, a, &a.dark, b(-0.4, 0.92, -0.66), b(-0.4, 0.98, -0.45), 0.05);
                    steering = p
                        .spawn((
                            Transform::from_translation(b(-0.4, 1.0, -0.42))
                                .with_rotation(Quat::from_rotation_x(1.05)),
                            Visibility::default(),
                        ))
                        .with_children(|w| {
                            w.spawn((
                                Mesh3d(a.wheel_ring.clone()),
                                MeshMaterial3d(a.dark.clone()),
                                Transform::from_scale(Vec3::new(1.0, 1.0, 1.0)),
                            ));
                            for k in 0..3 {
                                bx(
                                    w,
                                    a,
                                    &a.dark,
                                    Vec3::ZERO,
                                    Vec3::new(0.36, 0.015, 0.03),
                                    Quat::from_rotation_y(k as f32 * 1.05 + 0.1),
                                );
                            }
                            bx(w, a, &a.chrome, Vec3::ZERO, Vec3::new(0.07, 0.03, 0.07), q0);
                        })
                        .id();
                    // seats
                    for s in [-1.0f32, 1.0] {
                        let x = s * 0.4;
                        bx(p, a, &a.seat, b(x, 0.5, 0.12), Vec3::new(0.5, 0.14, 0.56), q0);
                        bx(p, a, &a.seat, b(x, 0.86, 0.42), Vec3::new(0.5, 0.62, 0.12), Quat::from_rotation_x(0.18));
                        bx(p, a, &a.seat, b(x, 1.23, 0.5), Vec3::new(0.26, 0.2, 0.1), q0);
                    }
                    bx(p, a, &a.seat, b(0.0, 0.5, 1.0), Vec3::new(1.5, 0.14, 0.5), q0);
                    bx(p, a, &a.seat, b(0.0, 0.82, 1.22), Vec3::new(1.5, 0.55, 0.1), Quat::from_rotation_x(0.12));
                    // seated driver (visible through the glass on AI cars)
                    driver = p
                        .spawn((Transform::from_translation(b(-0.4, 0.0, 0.0)), Visibility::default()))
                        .with_children(|d| {
                            let torso = Vec3::new(0.0, 1.0 - BODY_PIVOT, 0.38);
                            bx(d, a, &driver_shirt, torso, Vec3::new(0.38, 0.52, 0.2), Quat::from_rotation_x(0.15));
                            d.spawn((
                                Mesh3d(a.sphere.clone()),
                                MeshMaterial3d(driver_skin.clone()),
                                Transform::from_translation(Vec3::new(0.0, 1.43 - BODY_PIVOT, 0.35))
                                    .with_scale(Vec3::new(0.085, 0.105, 0.095)),
                            ));
                            bx(d, a, &driver_shirt, Vec3::new(0.0, 0.7 - BODY_PIVOT, 0.0), Vec3::new(0.36, 0.14, 0.5), q0);
                            for sx in [-0.2f32, 0.2] {
                                bar(
                                    d,
                                    a,
                                    &driver_shirt,
                                    Vec3::new(sx, 1.18 - BODY_PIVOT, 0.35),
                                    Vec3::new(sx * 0.5, 1.06 - BODY_PIVOT, -0.1),
                                    0.08,
                                );
                            }
                        })
                        .id();
                })
                .id();

            // wheels
            let spots = [
                (-TRACK, AXLE_F, -1.0f32),
                (TRACK, AXLE_F, 1.0),
                (-TRACK, AXLE_R, -1.0),
                (TRACK, AXLE_R, 1.0),
            ];
            for (i, (x, z, side)) in spots.into_iter().enumerate() {
                pivots[i] = r
                    .spawn((Transform::from_xyz(x, WHEEL_R, z), Visibility::default()))
                    .with_children(|w| {
                        spins[i] = w
                            .spawn((Transform::default(), Visibility::default()))
                            .with_children(|s| {
                                s.spawn((
                                    Mesh3d(a.tire.clone()),
                                    MeshMaterial3d(a.tire_m.clone()),
                                    Transform::from_rotation(Quat::from_rotation_z(FRAC_PI_2)),
                                ));
                                s.spawn((
                                    Mesh3d(a.rim.clone()),
                                    MeshMaterial3d(a.rim_m.clone()),
                                    Transform::from_rotation(Quat::from_rotation_z(FRAC_PI_2)),
                                ));
                                let xo = side * 0.118;
                                s.spawn((
                                    Mesh3d(a.hub.clone()),
                                    MeshMaterial3d(a.chrome.clone()),
                                    Transform::from_xyz(xo, 0.0, 0.0)
                                        .with_rotation(Quat::from_rotation_z(FRAC_PI_2)),
                                ));
                                for k in 0..5 {
                                    bx(
                                        s,
                                        a,
                                        &a.dark,
                                        Vec3::new(xo, 0.0, 0.0),
                                        Vec3::new(0.012, 0.4, 0.05),
                                        Quat::from_rotation_x(k as f32 * TAU / 10.0),
                                    );
                                }
                                // red valve cap: makes the rotation obvious
                                bx(
                                    s,
                                    a,
                                    &a.plate,
                                    Vec3::new(xo, 0.2, 0.0),
                                    Vec3::new(0.014, 0.03, 0.03),
                                    q0,
                                );
                            })
                            .id();
                    })
                    .id();
            }
        })
        .insert(Vehicle {
            body,
            pivots,
            spins,
            steering,
            driver,
            tail_mat,
            speed: 0.0,
            steer: 0.0,
            spin: 0.0,
            pitch: 0.0,
            roll: 0.0,
            braking: false,
            shown_braking: false,
            lock_rear: false,
        })
        .id();
    root
}

// ---------------------------------------------------------------- physics ---

fn step_car(
    car: &mut Car,
    veh: &mut Vehicle,
    tf: &mut Transform,
    throttle: f32,
    steer_in: f32,
    hb: bool,
    dt: f32,
) {
    let f3 = tf.forward();
    let fwd = Vec2::new(f3.x, f3.z).normalize_or_zero();
    let right = Vec2::new(-fwd.y, fwd.x);
    let vl = car.vel.dot(fwd);
    let vt0 = car.vel.dot(right);

    // --- longitudinal
    let mut braking = false;
    let a = if throttle > 0.0 {
        if vl < -0.5 {
            braking = true;
            16.0
        } else {
            9.5 * (1.0 - (vl / 45.0).clamp(0.0, 1.0))
        }
    } else if throttle < 0.0 {
        if vl > 0.5 {
            braking = true;
            -19.0
        } else {
            -5.0 * (1.0 - (-vl / 9.0).clamp(0.0, 1.0))
        }
    } else {
        -vl.signum() * (1.4 + 0.0009 * vl * vl) * (vl.abs() > 0.05) as i32 as f32
    };
    let mut a = a;
    if hb && vl.abs() > 0.1 {
        braking = true;
    }
    let mut nvl = vl + a * dt;
    if (throttle == 0.0 || braking) && vl != 0.0 && nvl.signum() != vl.signum() {
        nvl = 0.0;
    }

    // --- lateral grip with saturation (breaks away under handbrake / hard cornering)
    let grip = if hb { 1.2 } else { 10.0 };
    let amax = if hb { 3.5 } else { 10.5 };
    let alat = (-vt0 * grip).clamp(-amax, amax);
    let mut nvt = vt0 + alat * dt;
    if nvt.signum() != vt0.signum() && vt0 != 0.0 {
        nvt = 0.0;
    }
    // turning the velocity vector costs a little speed
    let sp0 = Vec2::new(nvl, vt0).length();
    let sp1 = Vec2::new(nvl, nvt).length();
    let loss = 0.35 * (vt0.abs().min(5.0) / 5.0);
    let (mut lvl, mut lvt) = (nvl, nvt);
    if sp1 > 1e-3 {
        let k = (sp0 * (1.0 - loss * dt)) / sp1;
        lvl *= k;
        lvt *= k;
    }
    if hb {
        // locked wheels: sliding friction acts against the whole velocity vector
        let spd = Vec2::new(lvl, lvt).length();
        let ns = if spd < 0.35 { 0.0 } else { (spd - 6.5 * dt).max(0.0) };
        if spd > 1e-4 {
            lvl *= ns / spd;
            lvt *= ns / spd;
        }
    }
    car.vel = fwd * lvl + right * lvt;
    tf.translation.x += car.vel.x * dt;
    tf.translation.z += car.vel.y * dt;

    // --- steering / yaw (bicycle model with grip limit => understeer; handbrake => oversteer)
    let target = steer_in * 0.58 / (1.0 + lvl.abs() * 0.05);
    veh.steer += (target - veh.steer) * smooth(9.0, dt);
    let mut yaw_t = lvl * veh.steer.tan() / WHEELBASE;
    if hb {
        yaw_t *= 1.7;
    } else {
        let cap = 9.5 / lvl.abs().max(1.0);
        yaw_t = yaw_t.clamp(-cap, cap);
    }
    car.yaw_rate += (yaw_t - car.yaw_rate) * smooth(8.0, dt);
    tf.rotate_y(car.yaw_rate * dt);

    // --- suspension: spring/damper driven by accelerations
    let acc = (lvl - vl) / dt.max(1e-4);
    car.acc_long += (acc - car.acc_long) * smooth(7.0, dt);
    let lat = lvl * car.yaw_rate;
    let pitch_t = car.acc_long * 0.0065;
    let roll_t = -lat * 0.0075;
    car.pitch_v += (-70.0 * (veh.pitch - pitch_t) - 9.0 * car.pitch_v) * dt;
    veh.pitch += car.pitch_v * dt;
    car.roll_v += (-70.0 * (veh.roll - roll_t) - 9.0 * car.roll_v) * dt;
    veh.roll += car.roll_v * dt;
    veh.pitch = veh.pitch.clamp(-0.12, 0.12);
    veh.roll = veh.roll.clamp(-0.14, 0.14);

    veh.speed = lvl;
    veh.braking = braking || (throttle < 0.0 && lvl > 0.5);
    veh.lock_rear = hb;
    car.slip = lvt.abs();
}

/// Body rectangle approximated by three circles.
fn circles(pos: Vec3, fwd: Vec2) -> [Vec2; 3] {
    let p = Vec2::new(pos.x, pos.z);
    [p - fwd * 1.45, p, p + fwd * 1.45]
}

pub fn car_physics(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    driving: Res<Driving>,
    colliders: Res<Colliders>,
    furniture: Res<Furniture>,
    mut cars: Query<(Entity, &mut Transform, &mut Car, &mut Vehicle)>,
    mut traffic: Query<(&Transform, &mut Traffic), Without<Car>>,
) {
    let dt = time.delta_secs().min(0.05);
    if dt <= 0.0 {
        return;
    }
    // 1. integrate each car
    for (e, mut tf, mut car, mut veh) in &mut cars {
        let driven = driving.0 == Some(e);
        let (th, st, hb) = if driven {
            (
                axis(&keys, KeyCode::KeyW, KeyCode::KeyS),
                axis(&keys, KeyCode::KeyA, KeyCode::KeyD),
                keys.pressed(KeyCode::Space),
            )
        } else {
            (0.0, 0.0, true)
        };
        car.input_throttle = th;
        step_car(&mut car, &mut veh, &mut tf, th, st, hb, dt);
        car.shake *= (1.0 - 6.0 * dt).max(0.0);
    }

    // 2. collisions: buildings, other cars, traffic
    let snapshot: Vec<(Entity, [Vec2; 3], Vec2)> = cars
        .iter()
        .map(|(e, tf, car, _)| {
            let f = tf.forward();
            (e, circles(tf.translation, Vec2::new(f.x, f.z).normalize_or_zero()), car.vel)
        })
        .collect();
    let mut impulses: Vec<(Entity, Vec2, Vec2)> = Vec::new(); // (entity, push, dv)

    for (e, mut tf, mut car, mut veh) in &mut cars {
        let f3 = tf.forward();
        let fwd = Vec2::new(f3.x, f3.z).normalize_or_zero();
        let mut total = Vec2::ZERO;
        let mut hit_n = Vec2::ZERO;
        let mut yaw_kick = 0.0;
        // buildings
        for (i, c) in circles(tf.translation, fwd).into_iter().enumerate() {
            let mut p = Vec3::new(c.x, 0.0, c.y);
            let mut n = push_out(&mut p, 1.0, &colliders.0);
            n += push_out(&mut p, 0.95, &furniture.0);
            if n != Vec2::ZERO {
                total += Vec2::new(p.x - c.x, p.z - c.y);
                hit_n += n;
                yaw_kick += (i as f32 - 1.0) * n.dot(Vec2::new(-fwd.y, fwd.x)) * 0.5;
            }
        }
        // other cars & traffic
        let my = circles(tf.translation, fwd);
        for (oe, oc, ov) in &snapshot {
            if *oe == e {
                continue;
            }
            let mut push = Vec2::ZERO;
            let mut nrm = Vec2::ZERO;
            for a in &my {
                for b in oc {
                    let d = *a - *b;
                    let l = d.length();
                    if l < 2.0 && l > 1e-4 {
                        push += d / l * (2.0 - l);
                        nrm += d / l;
                    }
                }
            }
            if nrm != Vec2::ZERO {
                let n = nrm.normalize();
                total += push * 0.5;
                let rel = car.vel - *ov;
                let closing = -rel.dot(n);
                if closing > 0.0 {
                    let imp = n * closing * 0.55;
                    car.vel += imp;
                    impulses.push((*oe, -push * 0.5, -imp));
                    car.shake = car.shake.max((closing * 0.05).min(0.6));
                }
            }
        }
        for (ttf, mut tr) in &mut traffic {
            let tf_fwd = Vec2::new(tr.dir.x, tr.dir.z);
            let oc = circles(ttf.translation, tf_fwd);
            let mut push = Vec2::ZERO;
            let mut nrm = Vec2::ZERO;
            for a in &my {
                for b in &oc {
                    let d = *a - *b;
                    let l = d.length();
                    if l < 2.0 && l > 1e-4 {
                        push += d / l * (2.0 - l);
                        nrm += d / l;
                    }
                }
            }
            if nrm != Vec2::ZERO {
                let n = nrm.normalize();
                total += push * 0.8;
                let closing = -(car.vel - tf_fwd * 8.0).dot(n);
                if closing > 0.0 {
                    car.vel += n * closing * 0.6;
                    car.shake = car.shake.max((closing * 0.05).min(0.6));
                }
                tr.hit = 2.5;
            }
        }
        tf.translation.x += total.x;
        tf.translation.z += total.y;
        if hit_n != Vec2::ZERO {
            let n = hit_n.normalize();
            let vn = car.vel.dot(n);
            if vn < 0.0 {
                car.vel -= n * vn * 1.3; // restitution 0.3
                car.vel *= 0.94;
                car.shake = car.shake.max((-vn * 0.06).min(0.8));
                car.yaw_rate += yaw_kick * (-vn).min(12.0) * 0.06;
            }
        }
        let mut p = tf.translation;
        clamp_world(&mut p);
        p.y = ground_h(p.x, p.z);
        tf.translation = p;
        veh.speed = car.vel.dot(fwd);
    }
    for (e, push, dv) in impulses {
        if let Ok((_, mut tf, mut car, _)) = cars.get_mut(e) {
            tf.translation.x += push.x;
            tf.translation.z += push.y;
            car.vel += dv;
        }
    }
}

/// Dark tyre marks when sliding or under the handbrake.
pub fn skid_marks(
    mut commands: Commands,
    time: Res<Time>,
    a: Res<CarAssets>,
    mut cars: Query<(&Transform, &mut Car, &Vehicle)>,
    mut marks: Query<(Entity, &mut Skid)>,
) {
    let dt = time.delta_secs();
    let mut count = 0;
    for (e, mut s) in &mut marks {
        s.life -= dt;
        count += 1;
        if s.life <= 0.0 {
            commands.entity(e).despawn();
        }
    }
    for (tf, mut car, veh) in &mut cars {
        car.skid_t -= dt;
        let sliding = car.slip > 2.2 || (veh.lock_rear && veh.speed.abs() > 4.0);
        if sliding && car.skid_t <= 0.0 && count < 600 {
            car.skid_t = 0.04;
            let vel = car.vel.normalize_or_zero();
            for x in [-TRACK, TRACK] {
                let pos = tf.transform_point(Vec3::new(x, 0.0, AXLE_R));
                let ang = f32::atan2(-vel.x, -vel.y);
                commands.spawn((
                    Mesh3d(a.skid_mesh.clone()),
                    MeshMaterial3d(a.skid_m.clone()),
                    Transform::from_xyz(pos.x, ground_h(pos.x, pos.z) + 0.012, pos.z)
                        .with_rotation(Quat::from_rotation_y(ang))
                        .with_scale(Vec3::new(0.22, 0.004, 0.75)),
                    Skid { life: 14.0 },
                ));
            }
        }
    }
}

// ---------------------------------------------------------------- traffic ---

pub fn traffic_ai(
    time: Res<Time>,
    signals: Res<Signals>,
    mut rng: ResMut<Rng>,
    driving: Res<Driving>,
    player: Query<&GlobalTransform, With<Player>>,
    cars: Query<(Entity, &Transform), (With<Car>, Without<Traffic>)>,
    mut ai: Query<(Entity, &mut Transform, &mut Traffic, &mut Vehicle), Without<Car>>,
) {
    let dt = time.delta_secs().min(0.05);
    // obstacle snapshot: (position, forward-ish radius)
    let mut obstacles: Vec<(Entity, Vec2)> = Vec::new();
    for (e, t) in &cars {
        obstacles.push((e, Vec2::new(t.translation.x, t.translation.z)));
    }
    if let (Ok(p), None) = (player.single(), driving.0) {
        let t = p.translation();
        if t.y < 1.5 {
            obstacles.push((Entity::PLACEHOLDER, Vec2::new(t.x, t.z)));
        }
    }
    for (e, t, _, _) in &ai {
        obstacles.push((e, Vec2::new(t.translation.x, t.translation.z)));
    }

    for (e, mut tf, mut tr, mut veh) in &mut ai {
        let pos = Vec2::new(tf.translation.x, tf.translation.z);
        let dir2 = Vec2::new(tr.dir.x, tr.dir.z);
        let perp = Vec2::new(-dir2.y, dir2.x);

        // nearest obstacle in the lane ahead
        let mut gap = 1e9f32;
        for (oe, op) in &obstacles {
            if *oe == e {
                continue;
            }
            let rel = *op - pos;
            let ahead = rel.dot(dir2);
            let lat = rel.dot(perp).abs();
            if ahead > 0.5 && ahead < 45.0 && lat < 2.2 {
                gap = gap.min(ahead - 4.6);
            }
        }
        // signal ahead
        let along_axis = if dir2.x.abs() > 0.5 { 1 } else { 0 };
        let q = if along_axis == 1 { pos.x } else { pos.y };
        let s = if along_axis == 1 { dir2.x } else { dir2.y }.signum();
        let state = signals.state(along_axis);
        let mut best = 1e9f32;
        for j in 0..=N {
            let stop = street_c(j) - s * 11.8;
            let d = (stop - q) * s;
            if d > -0.8 && d < best {
                best = d;
            }
        }
        if state == 2 && best < 60.0 {
            gap = gap.min(best.max(0.0));
        } else if state == 1 && best < 60.0 && best > veh.speed * veh.speed / 10.0 {
            gap = gap.min(best.max(0.0));
        }

        let v = veh.speed;
        let v_safe = (2.0 * 5.0 * (gap - 1.0).max(0.0)).sqrt();
        let mut target = tr.desired.min(v_safe);
        if tr.hit > 0.0 {
            tr.hit -= dt;
            target = 0.0;
        }
        let dv = if v < target { 3.2 * dt } else { -(8.0 + (v - target) * 1.5).min(14.0) * dt };
        let nv = (v + dv).max(0.0);
        tr.acc += ((nv - v) / dt.max(1e-4) - tr.acc) * smooth(6.0, dt);
        veh.speed = nv;
        veh.braking = tr.acc < -1.5;
        veh.steer = 0.0;
        veh.pitch += (tr.acc * 0.005 - veh.pitch) * smooth(6.0, dt);
        veh.roll = 0.0;

        tf.translation += tr.dir * nv * dt;
        let p = &mut tf.translation;
        if p.x > HALF {
            p.x -= 2.0 * HALF;
        } else if p.x < -HALF {
            p.x += 2.0 * HALF;
        }
        if p.z > HALF {
            p.z -= 2.0 * HALF;
        } else if p.z < -HALF {
            p.z += 2.0 * HALF;
        }

        if nv < 0.3 && best > 14.0 {
            tr.stuck += dt;
            if tr.stuck > 12.0 {
                // give up and re-enter at the far edge of the map
                tr.stuck = 0.0;
                if along_axis == 1 {
                    p.x = -s * (HALF - 4.0);
                } else {
                    p.z = -s * (HALF - 4.0);
                }
                tr.desired = rng.range(8.0, 14.0);
            }
        } else {
            tr.stuck = 0.0;
        }
    }
}

// -------------------------------------------------------------- animation ---

pub fn animate_vehicles(
    time: Res<Time>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut vs: Query<&mut Vehicle>,
    mut tfs: Query<&mut Transform, Without<Vehicle>>,
) {
    let dt = time.delta_secs();
    for mut v in &mut vs {
        v.spin = (v.spin + v.speed * dt / WHEEL_R) % (TAU * 1000.0);
        let spin = v.spin;
        let steer = v.steer;
        let (body, pivots, spins, st) = (v.body, v.pivots, v.spins, v.steering);
        for i in 0..4 {
            if let Ok(mut t) = tfs.get_mut(spins[i]) {
                let s = if i >= 2 && v.lock_rear { 0.0 } else { spin };
                if !(i >= 2 && v.lock_rear) {
                    t.rotation = Quat::from_rotation_x(-s);
                }
            }
            if i < 2 {
                if let Ok(mut t) = tfs.get_mut(pivots[i]) {
                    t.rotation = Quat::from_rotation_y(steer);
                }
            }
        }
        if let Ok(mut t) = tfs.get_mut(body) {
            t.rotation = Quat::from_rotation_z(v.roll) * Quat::from_rotation_x(v.pitch);
        }
        if let Ok(mut t) = tfs.get_mut(st) {
            t.rotation = Quat::from_rotation_x(1.05) * Quat::from_rotation_y(steer * 4.6);
        }
        if v.braking != v.shown_braking {
            v.shown_braking = v.braking;
            if let Some(m) = mats.get_mut(&v.tail_mat) {
                m.emissive = if v.braking {
                    LinearRgba::rgb(14.0, 0.5, 0.4)
                } else {
                    LinearRgba::rgb(0.5, 0.02, 0.02)
                };
            }
        }
    }
}
