//! Player on foot, entering/exiting cars, and the camera rig (first person, third person, cockpit, chase).

use crate::common::*;
use crate::person::Humanoid;
use crate::vehicle::{Car, Vehicle, DRIVER_EYE};
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow};
use std::f32::consts::{PI, TAU};

#[derive(Component)]
pub struct PlayerBody {
    pub vel: Vec2,
    pub vy: f32,
    pub grounded: bool,
}

#[derive(Resource)]
pub struct CamRig {
    pub yaw: f32,       // world yaw on foot
    pub pitch: f32,     // + = look up
    pub rel_yaw: f32,   // look offset relative to the car
    pub first_person: bool,
    pub dist: f32,
    pub car_dist: f32,
    pub locked: bool,
    pub idle: f32,
    pub car_yaw: f32,
    pub free: Option<(Vec3, Vec3, f32)>, // debug / screenshot camera
    pub bob_t: f32,
    pub fov: f32,
    pub inited: bool,
}

impl Default for CamRig {
    fn default() -> Self {
        Self {
            yaw: -0.45,
            pitch: 0.0,
            rel_yaw: 0.0,
            first_person: true,
            dist: 4.0,
            car_dist: 9.0,
            locked: false,
            idle: 0.0,
            car_yaw: 0.0,
            free: None,
            bob_t: 0.0,
            fov: 1.2,
            inited: false,
        }
    }
}

#[derive(Resource, Default)]
pub struct Prompt(pub Option<String>);

pub fn cursor_grab(
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut rig: ResMut<CamRig>,
) {
    let Ok(mut w) = windows.single_mut() else {
        return;
    };
    if keys.just_pressed(KeyCode::Escape) {
        rig.locked = false;
    }
    if mouse.just_pressed(MouseButton::Left) && w.focused {
        rig.locked = true;
    }
    if !rig.inited && w.focused {
        rig.inited = true;
        rig.locked = true;
    }
    let want = if rig.locked {
        CursorGrabMode::Locked
    } else {
        CursorGrabMode::None
    };
    if w.cursor_options.grab_mode != want {
        w.cursor_options.grab_mode = want;
        w.cursor_options.visible = !rig.locked;
    }
}

fn vehicle_circles(tf: &Transform) -> [Vec2; 3] {
    let f = tf.forward();
    let fwd = Vec2::new(f.x, f.z).normalize_or_zero();
    let p = Vec2::new(tf.translation.x, tf.translation.z);
    [p - fwd * 1.45, p, p + fwd * 1.45]
}

#[allow(clippy::type_complexity)]
pub fn interact(
    keys: Res<ButtonInput<KeyCode>>,
    mut rig: ResMut<CamRig>,
    mut driving: ResMut<Driving>,
    mut prompt: ResMut<Prompt>,
    colliders: Res<Colliders>,
    mut player: Query<(&mut Transform, &mut Visibility, &mut PlayerBody), (With<Player>, Without<Car>)>,
    mut cars: Query<(Entity, &Transform, &Vehicle, &mut Car), Without<Player>>,
    mut vis: Query<&mut Visibility, Without<Player>>,
) {
    let Ok((mut ptf, mut pvis, mut pb)) = player.single_mut() else {
        return;
    };
    prompt.0 = None;
    if driving.0.is_none() {
        let ppos = Vec2::new(ptf.translation.x, ptf.translation.z);
        let mut best: Option<(Entity, f32)> = None;
        for (e, t, _, _) in &cars {
            let d = vehicle_circles(t)
                .iter()
                .map(|c| c.distance(ppos))
                .fold(f32::MAX, f32::min);
            if d < 2.9 && best.map_or(true, |(_, bd)| d < bd) {
                best = Some((e, d));
            }
        }
        if best.is_some() {
            prompt.0 = Some("[F]  Get in car".into());
        }
        if keys.just_pressed(KeyCode::KeyF) {
            if let Some((e, _)) = best {
                driving.0 = Some(e);
                *pvis = Visibility::Hidden;
                rig.rel_yaw = 0.0;
                rig.pitch = 0.0;
                rig.idle = 0.0;
                if let Ok((_, _, veh, _car)) = cars.get_mut(e) {
                    if let Ok(mut v) = vis.get_mut(veh.driver) {
                        *v = Visibility::Hidden;
                    }
                }
                pb.vel = Vec2::ZERO;
            }
        }
    } else {
        if let Some(e) = driving.0 {
            if cars.get(e).map(|c| c.2.speed.abs() < 1.5).unwrap_or(true) {
                prompt.0 = Some("[F]  Get out".into());
            }
        }
        if keys.just_pressed(KeyCode::KeyF) {
            if let Some(e) = driving.0 {
                if let Ok((_, t, veh, _)) = cars.get(e) {
                    // pick the first free side
                    let mut chosen = t.translation + t.left() * 2.1;
                    for off in [t.left() * 2.1, t.right() * 2.1, t.left() * 2.8, t.back() * 3.6] {
                        let mut p = t.translation + off;
                        let n = push_out(&mut p, 0.4, &colliders.0);
                        if n == Vec2::ZERO {
                            chosen = p;
                            break;
                        }
                    }
                    ptf.translation = Vec3::new(chosen.x, ground_h(chosen.x, chosen.z), chosen.z);
                    let (yaw, _, _) = t.rotation.to_euler(EulerRot::YXZ);
                    rig.yaw = yaw;
                    rig.pitch = 0.0;
                    pb.vel = Vec2::ZERO;
                    pb.vy = 0.0;
                    pb.grounded = true;
                    if let Ok(mut v) = vis.get_mut(veh.driver) {
                        *v = Visibility::Inherited;
                    }
                }
            }
            driving.0 = None;
            *pvis = Visibility::Inherited;
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn look_input(
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    driving: Res<Driving>,
    mut rig: ResMut<CamRig>,
) {
    let dt = time.delta_secs();
    if keys.just_pressed(KeyCode::KeyV) {
        rig.first_person = !rig.first_person;
    }
    let mut d = Vec2::ZERO;
    if rig.locked {
        d = motion.delta * 0.0023;
    }
    if keys.pressed(KeyCode::KeyQ) {
        d.x -= 1.8 * dt;
    }
    if keys.pressed(KeyCode::KeyE) {
        d.x += 1.8 * dt;
    }
    let moved = d.length_squared() > 1e-8;
    if driving.0.is_some() {
        rig.rel_yaw = (rig.rel_yaw - d.x).clamp(-2.6, 2.6);
        rig.pitch = (rig.pitch - d.y).clamp(-1.2, 1.2);
        if moved {
            rig.idle = 0.0;
        } else {
            rig.idle += dt;
        }
        if rig.idle > 1.6 {
            let k = smooth(1.8, dt);
            rig.rel_yaw -= rig.rel_yaw * k;
            rig.pitch -= rig.pitch * k;
        }
    } else {
        rig.yaw -= d.x;
        rig.pitch = (rig.pitch - d.y).clamp(-1.5, 1.5);
    }
    if scroll.delta.y != 0.0 {
        if driving.0.is_some() {
            rig.car_dist = (rig.car_dist - scroll.delta.y * 0.8).clamp(5.5, 18.0);
        } else {
            rig.dist = (rig.dist - scroll.delta.y * 0.35).clamp(0.9, 12.0);
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn walk(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut rig: ResMut<CamRig>,
    driving: Res<Driving>,
    colliders: Res<Colliders>,
    furniture: Res<Furniture>,
    vehicles: Query<&Transform, (With<Vehicle>, Without<Player>)>,
    mut player: Single<(&mut Transform, &mut PlayerBody), With<Player>>,
) {
    if driving.0.is_some() {
        return;
    }
    let dt = time.delta_secs().min(0.05);
    let (tf, pb) = &mut *player;
    let inp = Vec3::new(
        axis(&keys, KeyCode::KeyD, KeyCode::KeyA),
        0.0,
        axis(&keys, KeyCode::KeyS, KeyCode::KeyW),
    );
    let yaw_q = Quat::from_rotation_y(rig.yaw);
    let dir = yaw_q * inp;
    let sprint = keys.pressed(KeyCode::ShiftLeft);
    let mut speed = if sprint { 6.2 } else { 2.5 };
    if inp.z > 0.0 {
        speed *= 0.65; // walking backwards
    }
    let target = if dir.length_squared() > 0.0 {
        let d = dir.normalize();
        Vec2::new(d.x, d.z) * speed
    } else {
        Vec2::ZERO
    };
    let rate = if pb.grounded { 11.0 } else { 1.5 };
    let cur = pb.vel;
    pb.vel = cur + (target - cur) * smooth(rate, dt);

    tf.translation.x += pb.vel.x * dt;
    tf.translation.z += pb.vel.y * dt;

    // facing: strafe-locked in first person, turn into the move direction in third person
    let face = if rig.first_person {
        Some(yaw_q)
    } else if pb.vel.length() > 0.3 {
        Some(yaw_facing(Vec3::new(pb.vel.x, 0.0, pb.vel.y)))
    } else {
        None
    };
    if let Some(q) = face {
        tf.rotation = tf.rotation.slerp(q, smooth(if rig.first_person { 16.0 } else { 9.0 }, dt));
    }

    // collisions: buildings + vehicles
    let mut p = tf.translation;
    push_out(&mut p, 0.36, &colliders.0);
    push_out(&mut p, 0.36, &furniture.0);
    for vt in &vehicles {
        for c in vehicle_circles(vt) {
            let d = Vec2::new(p.x, p.z) - c;
            let l = d.length();
            if l < 1.3 && l > 1e-4 && p.y < 1.4 {
                let push = d / l * (1.3 - l);
                p.x += push.x;
                p.z += push.y;
            }
        }
    }
    clamp_world(&mut p);

    // vertical: gravity, jump, smooth curb step
    let g = ground_h(p.x, p.z);
    if pb.grounded {
        if keys.just_pressed(KeyCode::Space) {
            pb.vy = 4.9;
            pb.grounded = false;
        } else {
            let dy = g - p.y;
            if dy > 0.0 {
                p.y += dy.min(3.2 * dt + dy * 0.3);
            } else {
                p.y = (p.y + dy * smooth(30.0, dt)).max(g);
                if dy < -0.4 {
                    pb.grounded = false;
                }
            }
        }
    }
    if !pb.grounded {
        pb.vy -= 14.0 * dt;
        p.y += pb.vy * dt;
        if p.y <= g {
            if g - p.y > 0.4 && pb.vy < 0.0 {
                // stepped into a wall-height step: treat as landing on the lower ground
            }
            p.y = g;
            pb.vy = 0.0;
            pb.grounded = true;
        }
    }
    tf.translation = p;
    rig.bob_t = rig.bob_t.max(0.0);
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn camera(
    time: Res<Time>,
    mut rig: ResMut<CamRig>,
    driving: Res<Driving>,
    colliders: Res<Colliders>,
    cars: Query<(&Transform, &Vehicle, &Car), Without<MainCamera>>,
    player: Single<(&Transform, &Humanoid), (With<Player>, Without<MainCamera>, Without<Car>)>,
    mut heads: Query<&mut Visibility>,
    mut cam: Single<(&mut Transform, &mut Projection), (With<MainCamera>, Without<Player>, Without<Car>)>,
) {
    let dt = time.delta_secs().min(0.05);
    let (ptf, hum) = &*player;
    let (ref mut ctf, ref mut proj) = *cam;

    let set_fov = |proj: &mut Projection, fov: f32| {
        if let Projection::Perspective(p) = proj {
            p.fov += (fov - p.fov) * smooth(6.0, dt);
        }
    };

    if let Some((pos, look, fov)) = rig.free {
        **ctf = Transform::from_translation(pos).looking_at(look, Vec3::Y);
        if let Projection::Perspective(p) = &mut **proj {
            p.fov = fov;
        }
        return;
    }

    // head visibility (hidden in first person so the camera never clips the skull)
    if let Some(h) = hum.j.head {
        if let Ok(mut v) = heads.get_mut(h) {
            let hide = rig.first_person && driving.0.is_none();
            let want = if hide { Visibility::Hidden } else { Visibility::Inherited };
            if *v != want {
                *v = want;
            }
        }
    }

    let t = time.elapsed_secs();
    let car = driving.0.and_then(|e| cars.get(e).ok());

    if let Some((ctf_car, veh, carc)) = car {
        let speed = veh.speed.abs();
        // smoothed car heading for the chase camera
        let cy = ctf_car.rotation.to_euler(EulerRot::YXZ).0;
        let diff = (cy - rig.car_yaw + PI).rem_euclid(TAU) - PI;
        rig.car_yaw += diff * smooth(if rig.first_person { 40.0 } else { 4.0 }, dt);
        let tilt = Quat::from_rotation_z(veh.roll * 0.7) * Quat::from_rotation_x(veh.pitch * 0.7);
        if rig.first_person {
            let pivot = Vec3::new(0.0, 0.45, 0.0);
            let local = pivot + tilt * (DRIVER_EYE - pivot);
            let shake = (speed * 0.0016 + carc.shake * 0.06).min(0.08);
            let jitter = Vec3::new(
                (t * 41.0).sin() * shake,
                (t * 53.0).sin() * shake * 0.8,
                (t * 37.0).cos() * shake * 0.4,
            );
            let pos = ctf_car.transform_point(local + jitter);
            ctf.translation = pos;
            ctf.rotation = ctf_car.rotation * tilt * Quat::from_euler(EulerRot::YXZ, rig.rel_yaw, rig.pitch - 0.07, 0.0);
            set_fov(proj, 1.3 + (speed * 0.006).min(0.25));
        } else {
            let target = ctf_car.translation + Vec3::Y * 1.5;
            let yaw = rig.car_yaw + rig.rel_yaw;
            let rot = Quat::from_euler(EulerRot::YXZ, yaw, rig.pitch - 0.2, 0.0);
            let dist = rig.car_dist + speed * 0.06;
            let desired = clip_camera(target, target + rot * Vec3::new(0.0, 0.0, dist), &colliders.0);
            let mut d = desired;
            d.y = d.y.max(0.6);
            ctf.translation = ctf.translation.lerp(d, smooth(9.0, dt));
            ctf.look_at(target + rot * Vec3::new(0.0, 0.0, -2.0) * 0.0, Vec3::Y);
            set_fov(proj, 1.15 + (speed * 0.005).min(0.2));
        }
        return;
    }

    // on foot
    let rot = Quat::from_euler(EulerRot::YXZ, rig.yaw, rig.pitch, 0.0);
    let sp = hum.speed;
    if rig.first_person {
        let sw = (sp / 2.5).clamp(0.0, 1.0);
        let run = ((sp - 3.0) / 3.0).clamp(0.0, 1.0);
        let bob_y = (2.0 * hum.phase).cos().abs() * -0.03 * sw * (1.0 + run) + 0.015 * sw;
        let bob_x = hum.phase.sin() * 0.012 * sw;
        let roll = hum.phase.sin() * 0.006 * sw;
        let yaw_q = Quat::from_rotation_y(rig.yaw);
        let eye = ptf.translation + yaw_q * Vec3::new(bob_x, 1.64 * hum.scale + bob_y, -0.11);
        ctf.translation = eye;
        ctf.rotation = rot * Quat::from_rotation_z(roll);
        set_fov(proj, 1.28 + run * 0.14);
    } else {
        let pivot = ptf.translation + Vec3::Y * (1.45 * hum.scale);
        let focus = pivot + rot * Vec3::new(0.42, 0.0, 0.0);
        let desired = clip_camera(pivot, pivot + rot * Vec3::new(0.42, 0.0, rig.dist), &colliders.0);
        let mut d = desired;
        d.y = d.y.max(0.3);
        ctf.translation = ctf.translation.lerp(d, smooth(20.0, dt));
        ctf.rotation = rot;
        let _ = focus;
        set_fov(proj, 1.1 + ((sp - 3.0) / 3.0).clamp(0.0, 1.0) * 0.12);
    }
}

/// Pulls a third-person camera in front of any building it would pass through.
fn clip_camera(from: Vec3, to: Vec3, cols: &[(Vec2, Vec2)]) -> Vec3 {
    let steps = 24;
    let mut last_ok = from;
    for i in 1..=steps {
        let p = from.lerp(to, i as f32 / steps as f32);
        let inside = cols.iter().any(|(c, h)| {
            (p.x - c.x).abs() < h.x + 0.35 && (p.z - c.y).abs() < h.y + 0.35 && p.y < 200.0
        });
        if inside {
            let back = (last_ok - from).length().max(1.0);
            return from + (last_ok - from).normalize_or_zero() * back;
        }
        last_ok = p;
    }
    to
}
