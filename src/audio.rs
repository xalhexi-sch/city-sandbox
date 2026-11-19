//! Fully procedural audio: every sound is synthesised at startup into an in-memory WAV.

use crate::common::*;
use crate::person::Humanoid;
use crate::vehicle::{Car, Vehicle};
use bevy::audio::{AudioPlayer, AudioSink, AudioSinkPlayback, AudioSource, PlaybackSettings, Volume};
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

const SR: usize = 44100;

#[derive(Component, Clone, Copy, PartialEq)]
pub enum Loop {
    Engine,
    Tires,
    Wind,
    Ambient,
    Horn,
}

#[derive(Resource)]
pub struct Sounds {
    pub step: Vec<Handle<AudioSource>>,
    pub thud: Handle<AudioSource>,
    pub door: Handle<AudioSource>,
}

#[derive(Default)]
pub struct AudioState {
    prev_driving: bool,
    prev_shake: f32,
    eng: f32,
    tire: f32,
    wind: f32,
    gear: usize,
}

// ------------------------------------------------------------- synthesis ---
fn wav(samples: &[f32]) -> Vec<u8> {
    let n = samples.len() as u32;
    let data = n * 2;
    let mut v = Vec::with_capacity(44 + data as usize);
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&(36 + data).to_le_bytes());
    v.extend_from_slice(b"WAVEfmt ");
    v.extend_from_slice(&16u32.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&(SR as u32).to_le_bytes());
    v.extend_from_slice(&((SR * 2) as u32).to_le_bytes());
    v.extend_from_slice(&2u16.to_le_bytes());
    v.extend_from_slice(&16u16.to_le_bytes());
    v.extend_from_slice(b"data");
    v.extend_from_slice(&data.to_le_bytes());
    for s in samples {
        v.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32000.0) as i16).to_le_bytes());
    }
    v
}

fn rnd(state: &mut u32) -> f32 {
    *state ^= *state << 13;
    *state ^= *state >> 17;
    *state ^= *state << 5;
    (*state as f32 / u32::MAX as f32) * 2.0 - 1.0
}

/// Sum of sinusoids at integer Hz so a 1 s buffer loops with no seam.
fn banded(f_lo: u32, f_hi: u32, count: u32, slope: f32, seed: u32, secs: usize) -> Vec<f32> {
    let n = SR * secs;
    let mut s = seed;
    let mut out = vec![0.0f32; n];
    for _ in 0..count {
        let f = f_lo as f32 + (rnd(&mut s) * 0.5 + 0.5) * (f_hi - f_lo) as f32;
        let f = (f / (1.0 / secs as f32)).round() * (1.0 / secs as f32);
        let a = (f_lo as f32 / f).powf(slope);
        let ph = (rnd(&mut s) * 0.5 + 0.5) * TAU;
        for (i, o) in out.iter_mut().enumerate() {
            *o += a * (TAU * f * i as f32 / SR as f32 + ph).sin();
        }
    }
    let peak = out.iter().fold(0.0f32, |m, v| m.max(v.abs())).max(1e-3);
    out.iter().map(|v| v / peak).collect()
}

fn engine_loop() -> Vec<u8> {
    let n = SR;
    let f0 = 40.0f32; // idle firing fundamental
    let mut s = 7u32;
    let mut out = vec![0.0f32; n];
    let amps = [0.9, 1.0, 0.8, 0.55, 0.5, 0.35, 0.3, 0.22, 0.18, 0.12, 0.1, 0.07];
    let phases: Vec<f32> = (0..amps.len()).map(|_| (rnd(&mut s) * 0.5 + 0.5) * TAU).collect();
    for (i, o) in out.iter_mut().enumerate() {
        let t = i as f32 / SR as f32;
        let pulse = 1.0 + 0.45 * (TAU * f0 * t).sin() + 0.2 * (TAU * 2.0 * f0 * t + 1.0).sin();
        let mut v = 0.0;
        for (k, (a, p)) in amps.iter().zip(&phases).enumerate() {
            let h = (k + 1) as f32;
            // soft-clipped waveform gives a growly exhaust note
            v += a * (TAU * f0 * h * t + p).sin();
        }
        *o = (v * 0.35 * pulse).tanh() * 0.8;
    }
    let grit = banded(60, 900, 60, 0.7, 11, 1);
    for (o, g) in out.iter_mut().zip(grit) {
        *o = *o * 0.85 + g * 0.12;
    }
    wav(&out)
}

fn tire_loop() -> Vec<u8> {
    let n = SR;
    let mut out = vec![0.0f32; n];
    let mut s = 3u32;
    let noise = banded(1800, 4200, 70, 0.0, 5, 1);
    // Accumulate phase per sample (the mean frequency is an integer number of Hz and the
    // LFOs are whole-Hz, so total phase over the 1 s loop is a whole number of cycles: no seam).
    let mut ph = 0.0f64;
    for (i, o) in out.iter_mut().enumerate() {
        let t = i as f32 / SR as f32;
        let f = 2300.0 + 220.0 * (TAU * 7.0 * t).sin() + 90.0 * (TAU * 13.0 * t).sin();
        ph += (std::f64::consts::TAU * f as f64) / SR as f64;
        let sq = 0.75 * (ph as f32).sin() + 0.25 * (2.0 * ph as f32 + 0.6).sin();
        let _ = rnd(&mut s);
        *o = 0.35 * sq * (0.7 + 0.3 * (TAU * 11.0 * t).sin()) + 0.25 * noise[i];
    }
    wav(&out)
}

fn wind_loop() -> Vec<u8> {
    let w = banded(70, 1400, 120, 0.9, 21, 2);
    wav(&w.iter().map(|v| v * 0.8).collect::<Vec<_>>())
}

fn ambient_loop() -> Vec<u8> {
    let low = banded(30, 220, 70, 1.1, 33, 4);
    let hiss = banded(900, 5200, 60, 1.0, 34, 4);
    let n = low.len();
    let mut out = vec![0.0f32; n];
    for i in 0..n {
        let t = i as f32 / SR as f32;
        let swell = 0.75 + 0.25 * (TAU * t / 4.0).sin();
        out[i] = (low[i] * 0.7 + hiss[i] * 0.12) * swell;
    }
    wav(&out)
}

fn horn_loop() -> Vec<u8> {
    let n = SR / 2;
    let mut out = vec![0.0f32; n];
    for (i, o) in out.iter_mut().enumerate() {
        let t = i as f32 / SR as f32;
        let mut v = 0.0;
        for (f, a) in [(440.0f32, 0.6f32), (554.0, 0.45), (660.0, 0.3)] {
            // square-ish: odd harmonics
            for h in [1.0f32, 3.0, 5.0] {
                v += a * (TAU * f * h * t).sin() / h;
            }
        }
        *o = (v * 0.5).tanh() * 0.7;
    }
    // 440*0.5 = 220 cycles, 554*0.5 = 277, 660*0.5 = 330 -> seamless
    wav(&out)
}

fn step_sound(seed: u32, pitch: f32) -> Vec<u8> {
    let n = (SR as f32 * 0.16) as usize;
    let mut s = seed;
    let mut lp = 0.0f32;
    let mut out = vec![0.0f32; n];
    for (i, o) in out.iter_mut().enumerate() {
        let t = i as f32 / SR as f32;
        let noise = rnd(&mut s);
        lp += (noise - lp) * 0.25 * pitch;
        let env_n = (-t * 42.0).exp();
        let thump = (TAU * 62.0 * pitch * t).sin() * (-t * 38.0).exp();
        *o = (lp * 1.3 * env_n + thump * 0.8) * 0.9;
    }
    wav(&out)
}

fn thud_sound() -> Vec<u8> {
    let n = (SR as f32 * 0.5) as usize;
    let mut s = 91u32;
    let mut lp = 0.0f32;
    let mut out = vec![0.0f32; n];
    for (i, o) in out.iter_mut().enumerate() {
        let t = i as f32 / SR as f32;
        lp += (rnd(&mut s) - lp) * 0.12;
        let body = (TAU * 48.0 * t * (1.0 - 0.3 * t)).sin() * (-t * 9.0).exp();
        let crunch = lp * (-t * 18.0).exp() * 1.4;
        *o = (body * 1.1 + crunch) * 0.9;
    }
    wav(&out)
}

fn door_sound() -> Vec<u8> {
    let n = (SR as f32 * 0.28) as usize;
    let mut s = 17u32;
    let mut lp = 0.0f32;
    let mut out = vec![0.0f32; n];
    for (i, o) in out.iter_mut().enumerate() {
        let t = i as f32 / SR as f32;
        lp += (rnd(&mut s) - lp) * 0.3;
        let thunk = (TAU * 110.0 * t).sin() * (-t * 22.0).exp();
        let click = lp * (-t * 60.0).exp();
        let latch = if t > 0.09 { (lp * 0.8) * (-(t - 0.09) * 70.0).exp() } else { 0.0 };
        *o = (thunk + click * 0.8 + latch * 0.6) * 0.8;
    }
    wav(&out)
}

// ----------------------------------------------------------------- setup ---
pub fn setup_audio(mut commands: Commands, mut assets: ResMut<Assets<AudioSource>>) {
    let mk = |assets: &mut Assets<AudioSource>, bytes: Vec<u8>| assets.add(AudioSource { bytes: bytes.into() });
    let looped = |commands: &mut Commands, h: Handle<AudioSource>, kind: Loop, vol: f32| {
        commands.spawn((
            AudioPlayer::<AudioSource>(h),
            PlaybackSettings::LOOP.with_volume(Volume::Linear(vol)),
            kind,
        ));
    };
    let eng = mk(&mut assets, engine_loop());
    let tire = mk(&mut assets, tire_loop());
    let wind = mk(&mut assets, wind_loop());
    let amb = mk(&mut assets, ambient_loop());
    let horn = mk(&mut assets, horn_loop());
    looped(&mut commands, eng, Loop::Engine, 0.0);
    looped(&mut commands, tire, Loop::Tires, 0.0);
    looped(&mut commands, wind, Loop::Wind, 0.0);
    looped(&mut commands, amb, Loop::Ambient, 0.2);
    looped(&mut commands, horn, Loop::Horn, 0.0);

    let steps: Vec<_> = (0..6)
        .map(|i| mk(&mut assets, step_sound(1000 + i * 77, 0.8 + 0.12 * i as f32)))
        .collect();
    commands.insert_resource(Sounds {
        step: steps,
        thud: mk(&mut assets, thud_sound()),
        door: mk(&mut assets, door_sound()),
    });
}

// ---------------------------------------------------------------- runtime ---
#[allow(clippy::too_many_arguments)]
pub fn update_audio(
    mut commands: Commands,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    driving: Res<Driving>,
    sounds: Res<Sounds>,
    mut rng: ResMut<Rng>,
    mut st: Local<AudioState>,
    cars: Query<(&Vehicle, &Car)>,
    mut sinks: Query<(&mut AudioSink, &Loop)>,
    mut humans: Query<(&mut Humanoid, &Transform, Option<&crate::player::PlayerBody>)>,
    cam: Single<&Transform, With<MainCamera>>,
) {
    let dt = time.delta_secs().min(0.1);
    let cp = cam.translation;
    let drv = driving.0.and_then(|e| cars.get(e).ok());

    // --- door sound on enter / exit
    let is_driving = drv.is_some();
    if is_driving != st.prev_driving {
        st.prev_driving = is_driving;
        commands.spawn((
            AudioPlayer::<AudioSource>(sounds.door.clone()),
            PlaybackSettings::DESPAWN.with_volume(Volume::Linear(0.7)),
        ));
    }

    // --- engine / tyres / wind targets
    let (mut eng_v, mut eng_p, mut tire_v, mut wind_v) = (0.0f32, 1.0f32, 0.0f32, 0.0f32);
    let mut horn = 0.0;
    if let Some((veh, car)) = drv {
        let speed = veh.speed.abs();
        let gears = [0.0f32, 9.0, 17.0, 26.0, 36.0, 50.0];
        let mut g = st.gear;
        while g + 1 < gears.len() - 1 && speed > gears[g + 1] {
            g += 1;
        }
        while g > 0 && speed < gears[g] - 2.0 {
            g -= 1;
        }
        st.gear = g;
        let rpm = ((speed - gears[g]) / (gears[g + 1] - gears[g])).clamp(0.0, 1.0);
        let thr = car.input_throttle.abs();
        eng_p = 0.85 + rpm * 1.9 + thr * 0.15;
        eng_v = 0.28 + 0.42 * thr + 0.12 * rpm;
        tire_v = ((car.slip - 2.0) / 6.0).clamp(0.0, 1.0) * 0.55;
        wind_v = (speed / 40.0).clamp(0.0, 1.0) * 0.45;
        if keys.pressed(KeyCode::KeyH) {
            horn = 0.45;
        }
        // impact thud
        if car.shake > 0.25 && st.prev_shake <= 0.25 {
            commands.spawn((
                AudioPlayer::<AudioSource>(sounds.thud.clone()),
                PlaybackSettings::DESPAWN.with_volume(Volume::Linear((car.shake * 1.4).min(1.0))),
            ));
        }
        st.prev_shake = car.shake;
    } else {
        st.prev_shake = 0.0;
    }
    let k = smooth(8.0, dt);
    st.eng += (eng_v - st.eng) * k;
    st.tire += (tire_v - st.tire) * k;
    st.wind += (wind_v - st.wind) * k;
    for (mut sink, kind) in &mut sinks {
        match kind {
            Loop::Engine => {
                sink.set_volume(Volume::Linear(st.eng));
                sink.set_speed(eng_p);
            }
            Loop::Tires => sink.set_volume(Volume::Linear(st.tire)),
            Loop::Wind => sink.set_volume(Volume::Linear(st.wind)),
            Loop::Ambient => sink.set_volume(Volume::Linear(if is_driving { 0.1 } else { 0.22 })),
            Loop::Horn => sink.set_volume(Volume::Linear(horn)),
        }
    }

    // --- footsteps, tied to the animated stride (heel strike every half cycle)
    for (mut h, tf, player) in &mut humans {
        if h.speed < 0.6 {
            continue;
        }
        let idx = ((h.phase - FRAC_PI_2) / PI).floor() as i32;
        if idx != h.step {
            h.step = idx;
            let d = tf.translation.distance(cp);
            let is_player = player.is_some();
            if is_driving && is_player {
                continue;
            }
            let loud = if is_player { 0.6 } else { 0.32 } * (1.0 + (h.speed / 6.0).min(1.0) * 0.6);
            let att = if is_player { 1.0 } else { (1.0 - d / 22.0).clamp(0.0, 1.0).powi(2) };
            if att > 0.02 {
                commands.spawn((
                    AudioPlayer::<AudioSource>(sounds.step[rng.pick(sounds.step.len())].clone()),
                    PlaybackSettings::DESPAWN.with_volume(Volume::Linear(loud * att)),
                ));
            }
        }
    }
}
