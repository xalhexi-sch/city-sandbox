//! World generation: roads, sidewalks, textured buildings, parks, street furniture, people,
//! plus signals and the day/night cycle.

use crate::common::*;
use crate::person::*;
use crate::player::*;
use crate::textures::*;
use crate::vehicle::*;
use bevy::core_pipeline::bloom::Bloom;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::pbr::{Atmosphere, AtmosphereSettings, CascadeShadowConfigBuilder};
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

// ------------------------------------------------------------ components ---
#[derive(Component)]
pub struct Ped {
    pub block: Vec2,
    pub s: f32,
    pub dir: f32,
    pub speed: f32,
    pub off: f32,
    pub pause: f32,
    pub timer: f32,
}

#[derive(Component)]
pub struct LampLight;

#[derive(Resource)]
pub struct NightMats {
    pub windows: Vec<Handle<StandardMaterial>>,
    pub shop: Handle<StandardMaterial>,
    pub lamp: Handle<StandardMaterial>,
    pub signals: [[Handle<StandardMaterial>; 3]; 2],
    pub last_sig: [u8; 2],
    pub last_k: f32,
}

#[derive(Resource)]
pub struct Lamps(pub Vec<Vec3>);

// --------------------------------------------------------------- meshes ---
fn quad_mesh(w: f32, d: f32, uv: impl Fn(f32, f32) -> [f32; 2]) -> Mesh {
    let (hx, hz) = (w / 2.0, d / 2.0);
    let pos = vec![[-hx, 0.0, -hz], [hx, 0.0, -hz], [hx, 0.0, hz], [-hx, 0.0, hz]];
    let uvs: Vec<[f32; 2]> = pos.iter().map(|p| uv(p[0], p[2])).collect();
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; 4])
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_inserted_indices(Indices::U32(vec![0, 2, 1, 0, 3, 2]))
}

/// Four-sided shaft with UVs that tile the facade atlas per floor/bay.
fn shaft_mesh(
    w: f32,
    d: f32,
    y0: f32,
    y1: f32,
    cells_x: i32,
    cells_y: i32,
    atlas_x: i32,
    atlas_y: i32,
    rng: &mut Rng,
) -> Mesh {
    let mut pos: Vec<[f32; 3]> = Vec::new();
    let mut nor: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut idx: Vec<u32> = Vec::new();
    for k in 0..4 {
        let a = k as f32 * FRAC_PI_2;
        let r = Vec3::new(a.cos(), 0.0, -a.sin());
        let n = Vec3::new(a.sin(), 0.0, a.cos());
        let (hw, hn, bays) = if k % 2 == 0 {
            (w / 2.0, d / 2.0, ((w / 3.0).round() as i32).max(1))
        } else {
            (d / 2.0, w / 2.0, ((d / 3.0).round() as i32).max(1))
        };
        let c = n * hn;
        let bl = c - r * hw + Vec3::Y * y0;
        let br = c + r * hw + Vec3::Y * y0;
        let tr = c + r * hw + Vec3::Y * y1;
        let tl = c - r * hw + Vec3::Y * y1;
        let base = pos.len() as u32;
        for p in [bl, br, tr, tl] {
            pos.push(p.to_array());
            nor.push(n.to_array());
        }
        let u0 = rng.pick(atlas_x as usize) as f32 / atlas_x as f32;
        let v0 = rng.pick(atlas_y as usize) as f32 / atlas_y as f32;
        let u1 = u0 + bays.min(cells_x) as f32 / atlas_x as f32;
        let v1 = v0 + cells_y as f32 / atlas_y as f32;
        uvs.extend_from_slice(&[[u0, v1], [u1, v1], [u1, v0], [u0, v0]]);
        idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nor)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_inserted_indices(Indices::U32(idx))
}

struct Env<'a> {
    cube: &'a Handle<Mesh>,
    cyl: &'a Handle<Mesh>,
    sphere: &'a Handle<Mesh>,
}

fn put(
    commands: &mut Commands,
    mesh: &Handle<Mesh>,
    m: &Handle<StandardMaterial>,
    pos: Vec3,
    scale: Vec3,
    rot: Quat,
) -> Entity {
    commands
        .spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(m.clone()),
            Transform {
                translation: pos,
                rotation: rot,
                scale,
            },
        ))
        .id()
}

#[allow(clippy::too_many_arguments)]
fn setup_camera(commands: &mut Commands) {
    commands.spawn((
        Camera3d::default(),
        Camera {
            hdr: true,
            ..default()
        },
        Projection::Perspective(PerspectiveProjection {
            fov: 1.25,
            near: 0.05,
            far: 4000.0,
            ..default()
        }),
        Tonemapping::TonyMcMapface,
        Bloom {
            intensity: 0.14,
            ..Bloom::NATURAL
        },
        Atmosphere::EARTH,
        AtmosphereSettings {
            aerial_view_lut_max_distance: 3000.0,
            ..default()
        },
        Transform::from_xyz(0.4, 1.7, 35.2),
        MainCamera,
    ));
}

// ----------------------------------------------------------------- world ---
#[allow(clippy::too_many_arguments)]
pub fn setup_world(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut rng: ResMut<Rng>,
    mut rig: ResMut<CamRig>,
) {
    setup_camera(&mut commands);

    // ---------- shared meshes / materials
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let cyl = meshes.add(Cylinder::new(1.0, 1.0).mesh().resolution(16));
    let sphere = meshes.add(Sphere::new(1.0).mesh().ico(2).unwrap());
    let env = Env {
        cube: &cube,
        cyl: &cyl,
        sphere: &sphere,
    };
    let plain = |mats: &mut Assets<StandardMaterial>, c: Color, r: f32, m: f32| {
        mats.add(StandardMaterial {
            base_color: c,
            perceptual_roughness: r,
            metallic: m,
            ..default()
        })
    };
    let tex = |mats: &mut Assets<StandardMaterial>, img: Handle<Image>, r: f32| {
        mats.add(StandardMaterial {
            base_color_texture: Some(img),
            perceptual_roughness: r,
            ..default()
        })
    };

    let grass_m = tex(&mut mats, images.add(grass()), 1.0);
    let walk_m = tex(&mut mats, images.add(sidewalk()), 0.9);
    let curb_m = tex(&mut mats, images.add(curb_stone()), 0.9);
    let gravel_m = tex(&mut mats, images.add(gravel()), 1.0);
    let road_img = images.add(road_segment());
    let road_m = mats.add(StandardMaterial {
        base_color_texture: Some(road_img),
        perceptual_roughness: 0.82,
        reflectance: 0.35,
        ..default()
    });
    let inter_m = mats.add(StandardMaterial {
        base_color_texture: Some(images.add(intersection())),
        perceptual_roughness: 0.82,
        reflectance: 0.35,
        ..default()
    });
    let concrete = plain(&mut mats, srgb(0.62, 0.61, 0.58), 0.9, 0.0);
    let metal = plain(&mut mats, srgb(0.55, 0.57, 0.6), 0.45, 0.7);
    let wood = plain(&mut mats, srgb(0.3, 0.2, 0.12), 0.9, 0.0);
    let pole_m = plain(&mut mats, srgb(0.16, 0.17, 0.18), 0.5, 0.6);
    let bark = plain(&mut mats, srgb(0.27, 0.19, 0.12), 0.95, 0.0);
    let greens: Vec<_> = [(0.12, 0.34, 0.12), (0.17, 0.42, 0.14), (0.1, 0.28, 0.13), (0.22, 0.45, 0.16)]
        .iter()
        .map(|&(r, g, b)| plain(&mut mats, srgb(r, g, b), 0.85, 0.0))
        .collect();
    let lamp_m = mats.add(StandardMaterial {
        base_color: srgb(1.0, 0.9, 0.6),
        emissive: LinearRgba::BLACK,
        ..default()
    });
    let red_blink = mats.add(StandardMaterial {
        base_color: srgb(1.0, 0.1, 0.05),
        emissive: LinearRgba::rgb(6.0, 0.2, 0.1),
        ..default()
    });
    let bench_m = plain(&mut mats, srgb(0.38, 0.26, 0.15), 0.8, 0.0);
    let path_m = plain(&mut mats, srgb(0.7, 0.66, 0.58), 0.95, 0.0);
    let awning_mats: Vec<_> = [
        ((0.75, 0.1, 0.1), (0.95, 0.92, 0.88)),
        ((0.1, 0.3, 0.55), (0.92, 0.92, 0.9)),
        ((0.12, 0.45, 0.25), (0.92, 0.9, 0.85)),
        ((0.9, 0.65, 0.12), (0.2, 0.2, 0.22)),
    ]
    .iter()
    .map(|&(a, b)| {
        tex(
            &mut mats,
            images.add(stripe_awning([a.0, a.1, a.2], [b.0, b.1, b.2])),
            0.85,
        )
    })
    .collect();

    // facades
    let mut window_mats: Vec<Handle<StandardMaterial>> = Vec::new();
    for k in 0..FACADE_KINDS {
        let spec = facade_spec(k);
        let (c, e) = facade(k);
        window_mats.push(mats.add(StandardMaterial {
            base_color_texture: Some(images.add(c)),
            emissive_texture: Some(images.add(e)),
            emissive: LinearRgba::BLACK,
            perceptual_roughness: spec.roughness,
            reflectance: if spec.roughness < 0.3 { 0.9 } else { 0.4 },
            ..default()
        }));
    }
    let (sc, se) = shopfront();
    let shop_m = mats.add(StandardMaterial {
        base_color_texture: Some(images.add(sc)),
        emissive_texture: Some(images.add(se)),
        emissive: LinearRgba::BLACK,
        perceptual_roughness: 0.55,
        ..default()
    });

    // ---------- ground
    commands.spawn((
        Mesh3d(meshes.add(quad_mesh(60000.0, 60000.0, |x, z| [x / 6.0, z / 6.0]))),
        MeshMaterial3d(grass_m.clone()),
        Transform::from_xyz(0.0, -0.02, 0.0),
    ));

    // ---------- roads
    let seg = meshes.add(quad_mesh(BLOCK, STREET, |x, z| [(8.0 - z) / 16.0, (x + 18.0) / 36.0]));
    // transposed for roads that run along z
    let seg_z = meshes.add(quad_mesh(STREET, BLOCK, |x, z| [(x + 8.0) / 16.0, (z + 18.0) / 36.0]));
    let inter = meshes.add(quad_mesh(STREET, STREET, |x, z| [(x + 8.0) / 16.0, (z + 8.0) / 16.0]));
    for j in 0..=N {
        let cj = street_c(j);
        for k in 0..=N {
            let ck = street_c(k);
            commands.spawn((
                Mesh3d(inter.clone()),
                MeshMaterial3d(inter_m.clone()),
                Transform::from_xyz(cj, 0.056, ck),
            ));
            if k < N {
                // segment along z between intersections (cj, ck) and (cj, ck+1)
                let zm = (ck + street_c(k + 1)) / 2.0;
                commands.spawn((
                    Mesh3d(seg_z.clone()),
                    MeshMaterial3d(road_m.clone()),
                    Transform::from_xyz(cj, 0.055, zm),
                ));
                // segment along x between intersections (ck, cj) and (ck+1, cj)
                commands.spawn((
                    Mesh3d(seg.clone()),
                    MeshMaterial3d(road_m.clone()),
                    Transform::from_xyz(zm, 0.055, cj),
                ));
            }
        }
    }

    // ---------- blocks
    let slab_body = meshes.add(Cuboid::new(BLOCK, CURB, BLOCK));
    let slab_top = meshes.add(quad_mesh(BLOCK, BLOCK, |x, z| [x / 2.0, z / 2.0]));
    let park_top = meshes.add(quad_mesh(LOT, LOT, |x, z| [x / 4.0, z / 4.0]));
    let tex_one = meshes.add(quad_mesh(1.0, 1.0, |x, z| [x + 0.5, z + 0.5]));
    let _ = tex_one;
    let trunk = meshes.add(Cylinder::new(0.2, 1.0).mesh().resolution(10));
    let ground_floor_h = 4.2;

    let mut colliders: Vec<(Vec2, Vec2)> = Vec::new();
    let mut centers: Vec<Vec2> = Vec::new();
    let mut lamps: Vec<Vec3> = Vec::new();
    let mut furn: Vec<(Vec2, Vec2)> = Vec::new();

    for i in 0..N {
        for k in 0..N {
            let cx = (i as f32 - (N - 1) as f32 / 2.0) * PITCH;
            let cz = (k as f32 - (N - 1) as f32 / 2.0) * PITCH;
            centers.push(Vec2::new(cx, cz));
            put(&mut commands, env.cube, &curb_m, Vec3::new(cx, CURB / 2.0, cz), Vec3::new(BLOCK, CURB, BLOCK), Quat::IDENTITY);
            let _ = &slab_body;
            commands.spawn((
                Mesh3d(slab_top.clone()),
                MeshMaterial3d(walk_m.clone()),
                Transform::from_xyz(cx, CURB + 0.002, cz),
            ));

            // street trees and lamps around the block edge
            for edge in 0..4 {
                let a = edge as f32 * FRAC_PI_2;
                let n = Vec3::new(a.sin(), 0.0, -a.cos()); // outward normal
                let t = Vec3::new(a.cos(), 0.0, a.sin()); // along the edge
                for &tt in &[-12.0f32, 12.0] {
                    let base = Vec3::new(cx, CURB, cz) + n * 17.6 + t * tt;
                    furn.push((Vec2::new(base.x, base.z), Vec2::splat(0.26)));
                    // tree
                    let hgt = rng.range(3.0, 4.2);
                    put(&mut commands, &trunk, &bark, base + Vec3::Y * hgt / 2.0, Vec3::new(1.0, hgt, 1.0), Quat::IDENTITY);
                    let gm = greens[rng.pick(4)].clone();
                    for _ in 0..3 {
                        let r = rng.range(1.0, 1.6);
                        let o = Vec3::new(rng.range(-0.7, 0.7), rng.range(0.0, 1.2), rng.range(-0.7, 0.7));
                        put(&mut commands, env.sphere, &gm, base + Vec3::Y * (hgt + 0.8) + o, Vec3::new(r, r * 0.85, r), Quat::IDENTITY);
                    }
                }
                for &tt in &[-6.0f32, 6.0] {
                    let base = Vec3::new(cx, CURB, cz) + n * 17.75 + t * tt;
                    furn.push((Vec2::new(base.x, base.z), Vec2::splat(0.12)));
                    put(&mut commands, env.cyl, &pole_m, base + Vec3::Y * 3.4, Vec3::new(0.07, 6.8, 0.07), Quat::IDENTITY);
                    let arm_end = base + n * 1.5 + Vec3::Y * 6.75;
                    let arm_mid = base + n * 0.75 + Vec3::Y * 6.75;
                    put(
                        &mut commands,
                        env.cube,
                        &pole_m,
                        arm_mid,
                        Vec3::new(0.07, 0.07, 1.5),
                        Quat::from_rotation_y(f32::atan2(n.x, n.z)),
                    );
                    put(&mut commands, env.sphere, &lamp_m, arm_end - Vec3::Y * 0.1, Vec3::new(0.22, 0.12, 0.22), Quat::IDENTITY);
                    lamps.push(arm_end - Vec3::Y * 0.3);
                }
            }

            // lots
            for (sx, sz) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                let lx = cx + sx * LOT_OFF;
                let lz = cz + sz * LOT_OFF;
                if rng.chance(0.15) {
                    // pocket park
                    commands.spawn((
                        Mesh3d(park_top.clone()),
                        MeshMaterial3d(grass_m.clone()),
                        Transform::from_xyz(lx, CURB + 0.02, lz),
                    ));
                    put(&mut commands, env.cube, &path_m, Vec3::new(lx, CURB + 0.03, lz), Vec3::new(1.6, 0.03, LOT), Quat::IDENTITY);
                    put(&mut commands, env.cube, &path_m, Vec3::new(lx, CURB + 0.031, lz), Vec3::new(LOT, 0.03, 1.6), Quat::IDENTITY);
                    for _ in 0..rng.pick(3) + 3 {
                        let tx = lx + rng.range(-6.5, 6.5);
                        let tz = lz + rng.range(-6.5, 6.5);
                        if (tx - lx).abs() < 1.2 || (tz - lz).abs() < 1.2 {
                            continue;
                        }
                        furn.push((Vec2::new(tx, tz), Vec2::splat(0.3)));
                        let hgt = rng.range(3.0, 5.5);
                        put(&mut commands, &trunk, &bark, Vec3::new(tx, CURB + hgt / 2.0, tz), Vec3::new(1.2, hgt, 1.2), Quat::IDENTITY);
                        let gm = greens[rng.pick(4)].clone();
                        for _ in 0..4 {
                            let r = rng.range(1.5, 2.5);
                            let o = Vec3::new(rng.range(-1.0, 1.0), rng.range(-0.3, 1.6), rng.range(-1.0, 1.0));
                            put(&mut commands, env.sphere, &gm, Vec3::new(tx, CURB + hgt + 0.6, tz) + o, Vec3::new(r, r * 0.8, r), Quat::IDENTITY);
                        }
                    }
                    // benches
                    for (bxo, bzo, rot) in [(0.0f32, 2.6f32, 0.0f32), (2.6, 0.0, FRAC_PI_2)] {
                        let c = Vec3::new(lx + bxo * (if sx > 0.0 { 1.0 } else { -1.0 }), CURB, lz + bzo * (if sz > 0.0 { 1.0 } else { -1.0 }));
                        let q = Quat::from_rotation_y(rot);
                        put(&mut commands, env.cube, &bench_m, c + Vec3::Y * 0.45, Vec3::new(1.6, 0.07, 0.45), q);
                        put(&mut commands, env.cube, &bench_m, c + q * Vec3::new(0.0, 0.8, 0.22), Vec3::new(1.6, 0.4, 0.06), q);
                        for lx2 in [-0.7f32, 0.7] {
                            put(&mut commands, env.cube, &pole_m, c + q * Vec3::new(lx2, 0.22, 0.0), Vec3::new(0.06, 0.44, 0.4), q);
                        }
                    }
                } else {
                    spawn_building(
                        &mut commands,
                        &mut meshes,
                        &env,
                        &mut rng,
                        Vec2::new(lx, lz),
                        ground_floor_h,
                        &window_mats,
                        &shop_m,
                        &concrete,
                        &gravel_m,
                        &metal,
                        &wood,
                        &red_blink,
                        &awning_mats,
                    );
                    colliders.push((Vec2::new(lx, lz), Vec2::splat(LOT / 2.0)));
                }
            }
        }
    }
    commands.insert_resource(Colliders(colliders));
    commands.insert_resource(BlockCenters(centers.clone()));
    commands.insert_resource(Lamps(lamps.clone()));

    // lamp light pool (real lights that hop to the nearest street lamps)
    for _ in 0..14 {
        commands.spawn((
            PointLight {
                intensity: 0.0,
                range: 42.0,
                radius: 0.3,
                color: Color::srgb(1.0, 0.82, 0.55),
                shadows_enabled: false,
                ..default()
            },
            Transform::from_xyz(0.0, -50.0, 0.0),
            LampLight,
        ));
    }

    // ---------- traffic signals
    let off = mats.add(StandardMaterial {
        base_color: srgb(0.05, 0.05, 0.05),
        ..default()
    });
    let sig_mats: [[Handle<StandardMaterial>; 3]; 2] = std::array::from_fn(|_| {
        std::array::from_fn(|i| {
            let c = [srgb(1.0, 0.1, 0.05), srgb(1.0, 0.7, 0.05), srgb(0.1, 1.0, 0.3)][i];
            mats.add(StandardMaterial {
                base_color: c,
                emissive: LinearRgba::BLACK,
                perceptual_roughness: 0.3,
                ..default()
            })
        })
    });
    for j in 0..=N {
        for k in 0..=N {
            let (cx, cz) = (street_c(j), street_c(k));
            // (corner sign x, corner sign z, signal axis, facing direction)
            let corners = [
                (1.0f32, 1.0f32, 0usize, Vec3::Z),
                (-1.0, -1.0, 0, Vec3::NEG_Z),
                (1.0, -1.0, 1, Vec3::X),
                (-1.0, 1.0, 1, Vec3::NEG_X),
            ];
            for (sx, sz, ax, face) in corners {
                let base = Vec3::new(cx + sx * 8.6, if ground_h(cx + sx * 9.0, cz + sz * 9.0) > 0.2 { CURB } else { ROAD_Y }, cz + sz * 8.6);
                furn.push((Vec2::new(base.x, base.z), Vec2::splat(0.12)));
                put(&mut commands, env.cyl, &pole_m, base + Vec3::Y * 2.6, Vec3::new(0.07, 5.2, 0.07), Quat::IDENTITY);
                let head_c = base + Vec3::Y * 4.6 + face * 0.2;
                let q = Quat::from_rotation_y(f32::atan2(face.x, face.z));
                put(&mut commands, env.cube, &off, head_c, Vec3::new(0.34, 1.0, 0.28), q);
                for l in 0..3 {
                    let p = head_c + Vec3::Y * (0.3 - 0.3 * l as f32) + face * 0.15;
                    put(&mut commands, env.sphere, &sig_mats[ax][l], p, Vec3::splat(0.11), Quat::IDENTITY);
                }
            }
        }
    }

    commands.insert_resource(NightMats {
        windows: window_mats,
        shop: shop_m,
        lamp: lamp_m,
        signals: sig_mats,
        last_sig: [9, 9],
        last_k: -1.0,
    });

    commands.insert_resource(Furniture(furn));

    // ---------- people
    let human_assets = HumanAssets::new(&mut meshes, &mut mats);
    for _ in 0..70 {
        let c = centers[rng.pick(centers.len())];
        let look = Look::random(&mut rng);
        let s = rng.range(0.0, 136.0);
        let e = spawn_humanoid(
            &mut commands,
            &human_assets,
            &mut mats,
            &look,
            Transform::from_xyz(c.x, CURB, c.y),
        );
        commands.entity(e).insert(Ped {
            block: c,
            s,
            dir: if rng.chance(0.5) { 1.0 } else { -1.0 },
            speed: rng.range(1.1, 1.9),
            off: rng.range(-0.7, 0.1),
            pause: 0.0,
            timer: rng.range(2.0, 12.0),
        });
    }

    // ---------- cars
    let car_assets = CarAssets::new(&mut meshes, &mut mats);
    let headlight = |p: &mut ChildSpawnerCommands| {
        p.spawn((
            SpotLight {
                intensity: 0.0,
                range: 80.0,
                inner_angle: 0.3,
                outer_angle: 0.7,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_xyz(0.0, 0.8, -2.3),
        ));
    };
    let skins = [srgb(0.9, 0.7, 0.58), srgb(0.6, 0.42, 0.3), srgb(0.4, 0.28, 0.2), srgb(0.95, 0.8, 0.7)];
    let shirts = [srgb(0.8, 0.8, 0.82), srgb(0.2, 0.3, 0.5), srgb(0.5, 0.15, 0.15), srgb(0.15, 0.15, 0.17)];

    // the player's own car, parked at the kerb next to where you start
    let start = spawn_vehicle(
        &mut commands,
        &car_assets,
        &mut mats,
        &car_assets.paints[1].clone(),
        Transform::from_xyz(2.5, ROAD_Y, 32.9).with_rotation(yaw_facing(Vec3::X)),
        (skins[0], shirts[0]),
    );
    commands.entity(start).insert((Car::default(), StartCar)).with_children(headlight);
    // parked cars at kerbs
    for _ in 0..14 {
        let j = rng.pick((N + 1) as usize) as i32;
        let c = street_c(j);
        let mut t = rng.range(-HALF + 20.0, HALF - 20.0);
        // keep away from intersections
        for kk in 0..=N {
            let ck = street_c(kk);
            if (t - ck).abs() < 12.0 {
                t = ck + 12.5 + rng.range(0.0, 4.0);
            }
        }
        let s = if rng.chance(0.5) { 1.0 } else { -1.0 };
        let (pos, dir) = if rng.chance(0.5) {
            (Vec3::new(t, ROAD_Y, c + s * 6.9), Vec3::new(s, 0.0, 0.0))
        } else {
            (Vec3::new(c - s * 6.9, ROAD_Y, t), Vec3::new(0.0, 0.0, s))
        };
        let paint = car_assets.paints[rng.pick(car_assets.paints.len())].clone();
        let e = spawn_vehicle(
            &mut commands,
            &car_assets,
            &mut mats,
            &paint,
            Transform::from_translation(pos).with_rotation(yaw_facing(dir)),
            (skins[rng.pick(4)], shirts[rng.pick(4)]),
        );
        commands.entity(e).insert(Car::default()).with_children(headlight);
    }
    // traffic
    for _ in 0..34 {
        let j = rng.pick((N + 1) as usize) as i32;
        let c = street_c(j);
        let t = rng.range(-HALF, HALF);
        let s = if rng.chance(0.5) { 1.0 } else { -1.0 };
        let (pos, dir) = if rng.chance(0.5) {
            (Vec3::new(t, ROAD_Y, c + s * 4.0), Vec3::new(s, 0.0, 0.0))
        } else {
            (Vec3::new(c - s * 4.0, ROAD_Y, t), Vec3::new(0.0, 0.0, s))
        };
        let paint = car_assets.paints[rng.pick(car_assets.paints.len())].clone();
        let e = spawn_vehicle(
            &mut commands,
            &car_assets,
            &mut mats,
            &paint,
            Transform::from_translation(pos).with_rotation(yaw_facing(dir)),
            (skins[rng.pick(4)], shirts[rng.pick(4)]),
        );
        commands.entity(e).insert(Traffic {
            dir,
            desired: rng.range(8.0, 14.0),
            stuck: 0.0,
            hit: 0.0,
            acc: 0.0,
        });
    }
    commands.insert_resource(car_assets);

    // ---------- player
    let pl = spawn_humanoid(
        &mut commands,
        &human_assets,
        &mut mats,
        &Look::player(),
        Transform::from_xyz(0.4, CURB, 35.2),
    );
    commands.entity(pl).insert((
        Player,
        PlayerBody {
            vel: Vec2::ZERO,
            vy: 0.0,
            grounded: true,
        },
    ));
    commands.insert_resource(human_assets);
    rig.yaw = -0.5;

    // ---------- sun
    commands.spawn((
        DirectionalLight {
            illuminance: 12000.0,
            shadows_enabled: true,
            ..default()
        },
        CascadeShadowConfigBuilder {
            num_cascades: 4,
            first_cascade_far_bound: 10.0,
            maximum_distance: 260.0,
            overlap_proportion: 0.25,
            ..default()
        }
        .build(),
        Transform::default(),
        Sun,
    ));
    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.7, 0.78, 1.0),
        brightness: 350.0,
        ..default()
    });
}

#[allow(clippy::too_many_arguments)]
fn spawn_building(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    env: &Env,
    rng: &mut Rng,
    c: Vec2,
    gf: f32,
    window_mats: &[Handle<StandardMaterial>],
    shop_m: &Handle<StandardMaterial>,
    concrete: &Handle<StandardMaterial>,
    gravel: &Handle<StandardMaterial>,
    metal: &Handle<StandardMaterial>,
    wood: &Handle<StandardMaterial>,
    red: &Handle<StandardMaterial>,
    awnings: &[Handle<StandardMaterial>],
) {
    let r = rng.next();
    let (h, kinds): (f32, &[usize]) = if r < 0.42 {
        (rng.range(9.0, 18.0), &[0, 6, 5, 1, 7])
    } else if r < 0.78 {
        (rng.range(18.0, 38.0), &[1, 3, 5, 7, 0, 3])
    } else {
        (rng.range(40.0, 82.0), &[2, 4, 3, 1, 2])
    };
    let kind = kinds[rng.pick(kinds.len())];
    let tower = h > 40.0;
    let has_shops = rng.chance(0.75);
    let wm = window_mats[kind].clone();
    let base_y = 0.3; // CURB
    let top_of_gf = base_y + gf;

    // ground floor
    let gm = if has_shops { shop_m.clone() } else { wm.clone() };
    let ground = if has_shops {
        shaft_mesh(LOT, LOT, base_y, top_of_gf, 5, 1, FAC_CELLS_X, 1, rng)
    } else {
        shaft_mesh(LOT, LOT, base_y, top_of_gf, 5, 1, FAC_CELLS_X, FAC_CELLS_Y, rng)
    };
    commands.spawn((
        Mesh3d(meshes.add(ground)),
        MeshMaterial3d(gm),
        Transform::from_xyz(c.x, 0.0, c.y),
    ));
    // cornice over the ground floor
    put(commands, env.cube, concrete, Vec3::new(c.x, top_of_gf + 0.18, c.y), Vec3::new(LOT + 0.5, 0.36, LOT + 0.5), Quat::IDENTITY);

    // awnings
    if has_shops {
        for k in 0..4 {
            let a = k as f32 * FRAC_PI_2;
            let n = Vec3::new(a.sin(), 0.0, a.cos());
            let rr = Vec3::new(a.cos(), 0.0, -a.sin());
            for &o in &[-3.75f32, 3.75] {
                if rng.chance(0.55) {
                    let m = &awnings[rng.pick(awnings.len())];
                    let p = Vec3::new(c.x, base_y + 3.35, c.y) + n * (LOT / 2.0 + 0.55) + rr * o;
                    put(
                        commands,
                        env.cube,
                        m,
                        p,
                        Vec3::new(4.2, 0.07, 1.3),
                        Quat::from_rotation_y(a) * Quat::from_rotation_x(0.28),
                    );
                }
            }
        }
    }

    // main shaft(s)
    let (shaft_top, setback) = if tower {
        (base_y + gf + 0.36 + (h - gf) * 0.62, true)
    } else {
        (base_y + h, false)
    };
    let y0 = top_of_gf + 0.36;
    let floors = (((shaft_top - y0) / 3.6).round() as i32).max(1);
    let shaft = shaft_mesh(LOT - 0.3, LOT - 0.3, y0, shaft_top, 5, floors, FAC_CELLS_X, FAC_CELLS_Y, rng);
    commands.spawn((
        Mesh3d(meshes.add(shaft)),
        MeshMaterial3d(wm.clone()),
        Transform::from_xyz(c.x, 0.0, c.y),
    ));
    // ledges every few floors on masonry styles
    if matches!(kind, 0 | 1 | 5 | 6 | 7) {
        let mut y = y0 + 3.6 * 3.0;
        while y < shaft_top - 1.0 {
            put(commands, env.cube, concrete, Vec3::new(c.x, y, c.y), Vec3::new(LOT + 0.1, 0.18, LOT + 0.1), Quat::IDENTITY);
            y += 3.6 * 3.0;
        }
    }
    // corner piers on masonry
    if matches!(kind, 1 | 7) {
        for (sx, sz) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
            put(
                commands,
                env.cube,
                concrete,
                Vec3::new(c.x + sx * (LOT / 2.0 - 0.1), (y0 + shaft_top) / 2.0, c.y + sz * (LOT / 2.0 - 0.1)),
                Vec3::new(0.5, shaft_top - y0, 0.5),
                Quat::IDENTITY,
            );
        }
    }

    let mut roof_w = LOT;
    let mut roof_y = shaft_top;
    if setback {
        // terrace slab, then a narrower upper tower
        put(commands, env.cube, concrete, Vec3::new(c.x, shaft_top + 0.15, c.y), Vec3::new(LOT, 0.3, LOT), Quat::IDENTITY);
        let top = base_y + h;
        let upper_w = 11.0;
        let uf = (((top - shaft_top) / 3.6).round() as i32).max(1);
        let upper = shaft_mesh(upper_w, upper_w, shaft_top + 0.3, top, 4, uf, FAC_CELLS_X, FAC_CELLS_Y, rng);
        commands.spawn((
            Mesh3d(meshes.add(upper)),
            MeshMaterial3d(wm.clone()),
            Transform::from_xyz(c.x, 0.0, c.y),
        ));
        roof_w = upper_w;
        roof_y = top;
        // terrace parapet
        for (sx, sz, w, d) in [(0.0f32, 1.0f32, LOT, 0.25f32), (0.0, -1.0, LOT, 0.25), (1.0, 0.0, 0.25, LOT), (-1.0, 0.0, 0.25, LOT)] {
            put(commands, env.cube, concrete, Vec3::new(c.x + sx * (LOT / 2.0 - 0.12), shaft_top + 0.7, c.y + sz * (LOT / 2.0 - 0.12)), Vec3::new(w, 0.8, d), Quat::IDENTITY);
        }
    }

    // roof slab + parapet
    put(commands, env.cube, gravel, Vec3::new(c.x, roof_y + 0.2, c.y), Vec3::new(roof_w - 0.2, 0.4, roof_w - 0.2), Quat::IDENTITY);
    for (sx, sz, w, d) in [
        (0.0f32, 1.0f32, roof_w + 0.3, 0.3f32),
        (0.0, -1.0, roof_w + 0.3, 0.3),
        (1.0, 0.0, 0.3, roof_w + 0.3),
        (-1.0, 0.0, 0.3, roof_w + 0.3),
    ] {
        put(
            commands,
            env.cube,
            concrete,
            Vec3::new(c.x + sx * (roof_w / 2.0), roof_y + 0.65, c.y + sz * (roof_w / 2.0)),
            Vec3::new(w, 0.9, d),
            Quat::IDENTITY,
        );
    }
    // rooftop machinery
    let span = roof_w / 2.0 - 2.5;
    for _ in 0..(1 + rng.pick(3)) {
        let (w, hh, d) = (rng.range(1.6, 3.0), rng.range(0.9, 1.6), rng.range(1.4, 2.4));
        let p = Vec3::new(c.x + rng.range(-span, span), roof_y + 0.4 + hh / 2.0, c.y + rng.range(-span, span));
        put(commands, env.cube, metal, p, Vec3::new(w, hh, d), Quat::from_rotation_y(rng.range(0.0, 0.2)));
        put(commands, env.cyl, metal, p + Vec3::Y * (hh / 2.0 + 0.05), Vec3::new(0.5, 0.1, 0.5), Quat::IDENTITY);
    }
    // stair / elevator bulkhead
    let bp = Vec3::new(c.x + rng.range(-span, span), roof_y + 0.4 + 1.3, c.y + rng.range(-span, span));
    put(commands, env.cube, concrete, bp, Vec3::new(3.0, 2.6, 3.0), Quat::IDENTITY);
    // wooden water tank on legs for mid-rise
    if !tower && rng.chance(0.5) {
        let tp = Vec3::new(c.x + rng.range(-span, span), roof_y + 0.4, c.y + rng.range(-span, span));
        for (lx, lz) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
            put(commands, env.cube, wood, tp + Vec3::new(lx * 0.9, 1.0, lz * 0.9), Vec3::new(0.12, 2.0, 0.12), Quat::IDENTITY);
        }
        put(commands, env.cyl, wood, tp + Vec3::Y * 3.0, Vec3::new(1.5, 2.2, 1.5), Quat::IDENTITY);
        put(commands, env.cyl, metal, tp + Vec3::Y * 4.15, Vec3::new(1.6, 0.1, 1.6), Quat::IDENTITY);
    }
    // antenna with aircraft light on towers
    if tower {
        let ap = Vec3::new(c.x + rng.range(-1.0, 1.0), roof_y + 0.4, c.y + rng.range(-1.0, 1.0));
        put(commands, env.cyl, metal, ap + Vec3::Y * 6.0, Vec3::new(0.09, 12.0, 0.09), Quat::IDENTITY);
        put(commands, env.sphere, red, ap + Vec3::Y * 12.1, Vec3::splat(0.22), Quat::IDENTITY);
        put(commands, env.cube, metal, ap + Vec3::Y * 8.0, Vec3::new(1.6, 0.06, 0.06), Quat::IDENTITY);
    }
}

// ---------------------------------------------------------------- people ---
fn ring_point(s: f32, off: f32) -> (Vec3, Vec3) {
    const R: f32 = 17.0;
    let s = s.rem_euclid(136.0);
    let edge = (s / 34.0) as i32 % 4;
    let t = s % 34.0 - 17.0;
    let (p, tan, outward) = match edge {
        0 => (Vec2::new(t, -R), Vec2::new(1.0, 0.0), Vec2::new(0.0, -1.0)),
        1 => (Vec2::new(R, t), Vec2::new(0.0, 1.0), Vec2::new(1.0, 0.0)),
        2 => (Vec2::new(-t, R), Vec2::new(-1.0, 0.0), Vec2::new(0.0, 1.0)),
        _ => (Vec2::new(-R, -t), Vec2::new(0.0, -1.0), Vec2::new(-1.0, 0.0)),
    };
    let p = p + outward * off;
    (Vec3::new(p.x, 0.0, p.y), Vec3::new(tan.x, 0.0, tan.y))
}

pub fn peds(time: Res<Time>, mut rng: ResMut<Rng>, mut q: Query<(&mut Transform, &mut Ped)>) {
    let dt = time.delta_secs().min(0.05);
    for (mut tf, mut ped) in &mut q {
        ped.timer -= dt;
        if ped.timer <= 0.0 {
            if ped.pause > 0.0 {
                ped.pause = 0.0;
                ped.timer = rng.range(6.0, 20.0);
            } else if rng.chance(0.45) {
                ped.pause = 1.0;
                ped.timer = rng.range(2.0, 6.0);
            } else {
                ped.timer = rng.range(6.0, 20.0);
                if rng.chance(0.15) {
                    ped.dir = -ped.dir;
                }
            }
        }
        if ped.pause == 0.0 {
            ped.s += ped.dir * ped.speed * dt;
        }
        let (p, tan) = ring_point(ped.s, ped.off);
        tf.translation = Vec3::new(ped.block.x + p.x, CURB, ped.block.y + p.z);
        if ped.pause == 0.0 {
            let target = yaw_facing(tan * ped.dir);
            tf.rotation = tf.rotation.slerp(target, smooth(8.0, dt));
        }
    }
}

// ------------------------------------------------------------ day / night ---
pub fn tick_signals(time: Res<Time>, mut s: ResMut<Signals>, mut nm: ResMut<NightMats>, mut mats: ResMut<Assets<StandardMaterial>>) {
    s.t += time.delta_secs();
    for ax in 0..2 {
        let st = s.state(ax);
        if nm.last_sig[ax] != st {
            nm.last_sig[ax] = st;
            for l in 0..3 {
                let on = match (st, l) {
                    (2, 0) | (1, 1) | (0, 2) => true,
                    _ => false,
                };
                if let Some(m) = mats.get_mut(&nm.signals[ax][l]) {
                    let c = m.base_color.to_linear();
                    m.emissive = if on {
                        LinearRgba::rgb(c.red * 9.0, c.green * 9.0, c.blue * 9.0)
                    } else {
                        LinearRgba::rgb(0.0, 0.0, 0.0)
                    };
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn daynight(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut clock: ResMut<Clock>,
    mut clear: ResMut<ClearColor>,
    mut ambient: ResMut<AmbientLight>,
    sun: Single<(&mut Transform, &mut DirectionalLight), With<Sun>>,
    mut heads: Query<&mut SpotLight>,
    mut nm: ResMut<NightMats>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    car_assets: Res<CarAssets>,
) {
    for (k, h) in [(KeyCode::Digit1, 8.0), (KeyCode::Digit2, 12.5), (KeyCode::Digit3, 18.4), (KeyCode::Digit4, 22.5)] {
        if keys.just_pressed(k) {
            clock.hours = h;
        }
    }
    if keys.just_pressed(KeyCode::KeyT) {
        clock.rate = if clock.rate < 0.1 { 0.6 } else if clock.rate < 1.0 { 0.0 } else { 0.05 };
    }
    clock.hours = (clock.hours + time.delta_secs() * clock.rate).rem_euclid(24.0);
    let elev = (clock.hours - 6.0) / 24.0 * TAU;
    let s = elev.sin();
    let day = (s * 3.0 + 0.45).clamp(0.0, 1.0);
    let night = 1.0 - day;
    let warm = (1.0 - (s.abs() * 4.5)).clamp(0.0, 1.0);

    let (mut tf, mut light) = sun.into_inner();
    let (angle, illum, col) = if s >= 0.0 {
        let w = warm;
        (
            elev,
            14000.0 * s.sqrt().max(0.0) * (1.0 - 0.5 * w),
            Color::srgb(1.0, 0.95 - 0.3 * w, 0.88 - 0.5 * w),
        )
    } else {
        (elev + PI, 900.0 * (-s).sqrt(), Color::srgb(0.6, 0.7, 1.0))
    };
    tf.rotation = Quat::from_rotation_y(0.6) * Quat::from_rotation_x(-angle.clamp(0.04, PI - 0.04));
    light.illuminance = illum;
    light.color = col;

    ambient.brightness = 45.0 + 330.0 * day;
    let lerpc = |a: Vec3, b: Vec3| a + (b - a) * day;
    let sky_day = Vec3::new(0.42, 0.66, 0.96);
    let sky_night = Vec3::new(0.012, 0.016, 0.04);
    let mut sky = lerpc(sky_night, sky_day);
    sky += Vec3::new(0.55, 0.22, 0.02) * warm * 0.45;
    clear.0 = Color::srgb(sky.x, sky.y, sky.z);

    for mut h in &mut heads {
        h.intensity = night * 450_000.0;
    }

    if (nm.last_k - night).abs() > 0.01 {
        nm.last_k = night;
        let k = night * night;
        for h in nm.windows.clone() {
            if let Some(m) = mats.get_mut(&h) {
                m.emissive = LinearRgba::rgb(2.4 * k + 0.0, 2.4 * k, 2.4 * k);
            }
        }
        if let Some(m) = mats.get_mut(&nm.shop) {
            let v = 0.25 * day + 3.0 * night;
            m.emissive = LinearRgba::rgb(v, v, v);
        }
        if let Some(m) = mats.get_mut(&nm.lamp) {
            let v = 14.0 * k;
            m.emissive = LinearRgba::rgb(v, v * 0.82, v * 0.5);
        }
        if let Some(m) = mats.get_mut(&car_assets.headlight) {
            let v = 0.3 + 8.0 * k;
            m.emissive = LinearRgba::rgb(v, v, v * 0.95);
        }
    }
}

/// Moves a small pool of real point lights onto the street lamps nearest the camera.
pub fn lamp_pool(
    clock: Res<Clock>,
    lamps: Res<Lamps>,
    cam: Single<&Transform, (With<MainCamera>, Without<LampLight>)>,
    mut lights: Query<(&mut Transform, &mut PointLight), With<LampLight>>,
) {
    let elev = (clock.hours - 6.0) / 24.0 * TAU;
    let day = (elev.sin() * 3.0 + 0.45).clamp(0.0, 1.0);
    let night = 1.0 - day;
    let cp = cam.translation;
    let mut near: Vec<(f32, Vec3)> = lamps.0.iter().map(|l| (l.distance_squared(cp), *l)).collect();
    near.sort_by(|a, b| a.0.total_cmp(&b.0));
    for ((mut tf, mut pl), (d2, pos)) in lights.iter_mut().zip(near.into_iter()) {
        tf.translation = pos;
        let fade = (1.0 - (d2.sqrt() / 70.0)).clamp(0.0, 1.0);
        pl.intensity = night * night * 380_000.0 * fade;
    }
}
