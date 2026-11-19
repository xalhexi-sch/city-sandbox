mod audio;
mod city;
mod common;
mod person;
mod player;
mod shots;
mod textures;
mod ui;
mod vehicle;

use bevy::prelude::*;
use common::*;

fn main() {
    let shot = shots::shot_mode();
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "City Sandbox".into(),
            resolution: (1280.0_f32, 720.0_f32).into(),
            ..default()
        }),
        ..default()
    }))
    .insert_resource(Rng(0x2545_F491_4F6C_DD1D))
    .insert_resource(Driving::default())
    .insert_resource(player::CamRig::default())
    .insert_resource(player::Prompt::default())
    .insert_resource(Signals::default())
    .insert_resource(Clock {
        hours: 9.5,
        rate: 0.05,
    })
    .insert_resource(ClearColor(Color::srgb(0.45, 0.68, 0.95)))
    .add_systems(Startup, (city::setup_world, ui::setup_ui, audio::setup_audio).chain())
    .add_systems(
        Update,
        (
            player::cursor_grab,
            player::look_input,
            player::interact,
            player::walk,
            vehicle::car_physics,
            vehicle::traffic_ai,
            city::peds,
            person::animate_humanoids,
            vehicle::animate_vehicles,
            vehicle::skid_marks,
            player::camera,
        )
            .chain(),
    )
    .add_systems(
        Update,
        (
            audio::update_audio,
            city::tick_signals,
            city::daynight,
            city::lamp_pool,
            ui::hud,
            ui::minimap,
        )
            .after(player::camera),
    );
    if shot {
        app.insert_resource(shots::ShotState::default())
            .add_systems(Update, shots::shots.before(player::cursor_grab));
    }
    app.run();
}
