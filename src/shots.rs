//! Debug aid: `CITY_SHOT=1 city-sandbox.exe` renders a scripted tour of the city (including
//! injected key presses to drive and run) to PNG files, prints telemetry, and exits.

use crate::common::*;
use crate::person::Humanoid;
use crate::player::{CamRig, PlayerBody};
use crate::vehicle::{Car, StartCar, Traffic, Vehicle};
use crate::player::Prompt;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use std::f32::consts::PI;

#[derive(Resource, Default)]
pub struct ShotState {
    pub frame: u32,
}

pub fn shot_mode() -> bool {
    std::env::var("CITY_SHOT").is_ok()
}

const STEPS: &[(u32, &str)] = &[
    (100, "shot_01_fp.png"),
    (160, "shot_02_third.png"),
    (230, "shot_03_street.png"),
    (300, "shot_04_aerial.png"),
    (380, "shot_05_night.png"),
    (460, "shot_06_cockpit.png"),
    (540, "shot_07_chase.png"),
    (640, "shot_08_wheel.png"),
    (700, "shot_09_facade.png"),
    (790, "shot_10_accel.png"),
    (845, "shot_11_drift.png"),
    (852, "shot_12_skids.png"),
    (1000, "shot_13_run_a.png"),
    (1007, "shot_13_run_b.png"),
    (1060, "shot_14_walk_fp.png"),
    (1095, "shot_15_prompt.png"),
    (1118, "shot_16_entered.png"),
    (1235, "shot_17_exited.png"),
    (1266, "shot_18_face_ingame.png"),
];

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn shots(
    mut commands: Commands,
    mut st: ResMut<ShotState>,
    mut rig: ResMut<CamRig>,
    mut clock: ResMut<Clock>,
    mut driving: ResMut<Driving>,
    mut exit: EventWriter<AppExit>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut car_q: Query<(Entity, &mut Transform, &mut Car, &Vehicle), (With<StartCar>, Without<PlayerBody>)>,
    mut player_q: Query<(Entity, &mut Transform, &mut PlayerBody, &Humanoid), (Without<Car>, Without<Traffic>)>,
    mut vis: Query<&mut Visibility>,
    time: Res<Time>,
    signals: Res<Signals>,
    prompt: Res<Prompt>,
    colliders: Res<Colliders>,
    traffic: Query<(&Vehicle, &Transform), (With<Traffic>, Without<Car>, Without<PlayerBody>)>,
) {
    st.frame += 1;
    let f = st.frame;
    let Ok((car_e, mut ctf, mut car, veh)) = car_q.single_mut() else {
        return;
    };
    let Ok((pl_e, mut ptf, mut pb, hum)) = player_q.single_mut() else {
        return;
    };
    clock.rate = 0.0;

    match f {
        1 => {
            clock.hours = 10.5;
            rig.first_person = true;
            rig.locked = false;
        }
        120 => {
            rig.first_person = false;
            rig.dist = 3.2;
            rig.yaw = PI + 0.55;
            rig.pitch = 0.08;
        }
        200 => rig.free = Some((Vec3::new(29.0, 3.2, 120.0), Vec3::new(26.0, 6.0, -100.0), 1.15)),
        270 => rig.free = Some((Vec3::new(-60.0, 120.0, 150.0), Vec3::new(10.0, 10.0, 20.0), 1.0)),
        330 => {
            clock.hours = 22.3;
            rig.free = Some((Vec3::new(-26.0, 2.4, 40.0), Vec3::new(-26.0, 5.0, -60.0), 1.3));
        }
        420 => {
            clock.hours = 17.0;
            rig.free = None;
            driving.0 = Some(car_e);
            if let Ok(mut v) = vis.get_mut(pl_e) {
                *v = Visibility::Hidden;
            }
            if let Ok(mut v) = vis.get_mut(veh.driver) {
                *v = Visibility::Hidden;
            }
            rig.first_person = true;
            rig.rel_yaw = 0.0;
            rig.pitch = 0.0;
            let f3 = ctf.forward();
            car.vel = Vec2::new(f3.x, f3.z) * 14.0;
        }
        500 => {
            rig.first_person = false;
            rig.pitch = 0.0;
            let f3 = ctf.forward();
            car.vel = Vec2::new(f3.x, f3.z) * 14.0;
        }
        570 => {
            driving.0 = None;
            car.vel = Vec2::ZERO;
        }
        580 => {
            let cam = ctf.transform_point(Vec3::new(2.4, 0.6, -2.3));
            let look = ctf.transform_point(Vec3::new(0.8, 0.33, -1.38));
            rig.free = Some((cam, look, 0.8));
        }
        660 => rig.free = Some((Vec3::new(40.0, 6.0, 10.0), Vec3::new(36.0, 14.0, 0.0), 0.9)),
        // ---- drive test on an open stretch of street
        720 => {
            rig.free = None;
            *ctf = Transform::from_xyz(-120.0, ROAD_Y, 30.0).with_rotation(yaw_facing(Vec3::X));
            car.vel = Vec2::ZERO;
            car.yaw_rate = 0.0;
            driving.0 = Some(car_e);
            rig.first_person = false;
            rig.rel_yaw = 0.0;
            rig.pitch = 0.0;
            rig.car_yaw = ctf.rotation.to_euler(EulerRot::YXZ).0;
            rig.car_dist = 8.0;
        }
        722 => keys.press(KeyCode::KeyW),
        820 => {
            keys.press(KeyCode::KeyA);
            keys.press(KeyCode::Space);
        }
        860 => {
            keys.release(KeyCode::KeyA);
            keys.release(KeyCode::Space);
            keys.release(KeyCode::KeyW);
        }
        847 => {
            let p = ctf.translation;
            rig.free = Some((p + Vec3::new(2.0, 16.0, 9.0), p + Vec3::new(-6.0, 0.0, 0.0), 1.0));
        }
        // ---- run test
        915 => {
            rig.free = None;
            driving.0 = None;
            if let Ok(mut v) = vis.get_mut(pl_e) {
                *v = Visibility::Inherited;
            }
            *ptf = Transform::from_xyz(-110.0, ROAD_Y, 22.0);
            pb.vel = Vec2::ZERO;
            rig.first_person = false;
            rig.yaw = -PI / 2.0;
            rig.pitch = 0.0;
            keys.press(KeyCode::KeyW);
            keys.press(KeyCode::ShiftLeft);
        }
        1030 => {
            rig.free = None;
            rig.first_person = true;
            keys.release(KeyCode::ShiftLeft);
        }
        // ---- real enter/exit path through interact(): F key from the spawn point
        1080 => {
            keys.release(KeyCode::KeyW);
            keys.release(KeyCode::ShiftLeft);
            rig.free = None;
            driving.0 = None;
            if let Ok(mut v) = vis.get_mut(pl_e) {
                *v = Visibility::Inherited;
            }
            if let Ok(mut v) = vis.get_mut(veh.driver) {
                *v = Visibility::Inherited;
            }
            *ctf = Transform::from_xyz(2.5, ROAD_Y, 32.9).with_rotation(yaw_facing(Vec3::X));
            car.vel = Vec2::ZERO;
            car.yaw_rate = 0.0;
            *ptf = Transform::from_xyz(0.4, CURB, 35.2);
            pb.vel = Vec2::ZERO;
            rig.first_person = true;
            rig.yaw = -0.5;
            rig.pitch = 0.0;
        }
        1096 => keys.press(KeyCode::KeyF),
        1097 => keys.release(KeyCode::KeyF),
        1116 => keys.press(KeyCode::KeyW),
        1161 => keys.release(KeyCode::KeyW),
        1163 => keys.press(KeyCode::Space),
        1215 => keys.release(KeyCode::Space),
        1220 => keys.press(KeyCode::KeyF),
        1221 => keys.release(KeyCode::KeyF),
        1230 => {
            rig.first_person = false;
            rig.dist = 4.0;
            rig.pitch = 0.1;
        }
        1252 => {
            let p = ptf.translation;
            let fwd = ptf.forward().as_vec3();
            rig.free = Some((p + fwd * 1.05 + Vec3::Y * 1.68, p + Vec3::Y * 1.66, 0.55));
        }
        _ => {}
    }

    if matches!(f, 1090 | 1118 | 1150 | 1218 | 1236) {
        let inside = colliders
            .0
            .iter()
            .any(|(c, h)| (ptf.translation.x - c.x).abs() < h.x && (ptf.translation.z - c.y).abs() < h.y);
        let driver_vis = vis.get(veh.driver).map(|v| format!("{:?}", *v)).unwrap_or_default();
        println!(
            "[enter/exit f={f}] driving={} prompt={:?} car_speed={:.1} player_pos=({:.2},{:.2},{:.2}) inside_building={inside} driver_mesh={driver_vis}",
            driving.0.is_some(),
            prompt.0,
            veh.speed,
            ptf.translation.x,
            ptf.translation.y,
            ptf.translation.z
        );
    }

    // free side-on camera that tracks the runner
    if (940..=1010).contains(&f) {
        let p = ptf.translation;
        rig.free = Some((p + Vec3::new(0.0, 1.0, 3.6), p + Vec3::new(0.0, 0.95, 0.0), 0.8));
    }

    if matches!(f, 780 | 835 | 860 | 990 | 1050) {
        println!(
            "[telemetry f={f}] car speed={:.1} m/s slip={:.2} yaw_rate={:.2} pitch={:.3} roll={:.3} steer={:.2} | runner speed={:.2} m/s phase={:.1}",
            veh.speed, car.slip, car.yaw_rate, veh.pitch, veh.roll, veh.steer, hum.speed, hum.phase
        );
        println!("   car pos=({:.2},{:.2}) vel=({:.3},{:.3}) fwd={:?}", ctf.translation.x, ctf.translation.z, car.vel.x, car.vel.y, ctf.forward());
    }
    if matches!(f, 150 | 400 | 650 | 1000) {
        let (mut moving, mut stopped, mut violators) = (0, 0, 0);
        for (v, t) in &traffic {
            if v.speed > 1.0 {
                moving += 1;
            } else if v.speed < 0.3 {
                stopped += 1;
            }
            // inside an intersection square while that axis shows red?
            for j in 0..=N {
                for k in 0..=N {
                    let (cx, cz) = (street_c(j), street_c(k));
                    let (dx, dz) = ((t.translation.x - cx).abs(), (t.translation.z - cz).abs());
                    if dx < 7.0 && dz < 7.0 {
                        let along_x = (t.rotation * Vec3::NEG_Z).x.abs() > 0.5;
                        let axis = if along_x { 1 } else { 0 };
                        if signals.state(axis) == 2 && v.speed > 2.0 {
                            violators += 1;
                        }
                    }
                }
            }
        }
        println!(
            "[traffic f={f}] t={:.1}s fps~{:.1} moving={moving} stopped={stopped} in-junction-on-red={violators} signal ns={} ew={}",
            time.elapsed_secs(),
            f as f32 / time.elapsed_secs().max(0.001),
            signals.state(0),
            signals.state(1)
        );
    }
    for (at, name) in STEPS {
        if f == *at {
            commands.spawn(Screenshot::primary_window()).observe(save_to_disk(*name));
        }
    }
    if f > STEPS.last().unwrap().0 + 15 {
        exit.write(AppExit::Success);
    }
}
