use crate::common::*;
use crate::player::{CamRig, Prompt};
use crate::vehicle::{Car, Traffic, Vehicle};
use bevy::prelude::*;

pub const MAP: f32 = 190.0;
const MAP_SCALE: f32 = 0.5;

#[derive(Component)]
pub struct HudClock;
#[derive(Component)]
pub struct HudSpeed;
#[derive(Component)]
pub struct HudSpeedUnit;
#[derive(Component)]
pub struct HudBar;
#[derive(Component)]
pub struct HudPrompt;
#[derive(Component)]
pub struct HudHelp;
#[derive(Component)]
pub struct Crosshair;
#[derive(Component)]
pub struct MapBlock(pub Vec2);
#[derive(Component)]
pub struct MapDot(pub Entity);
#[derive(Component)]
pub struct MapPlayer;

pub fn setup_ui(
    mut commands: Commands,
    blocks: Res<BlockCenters>,
    cars: Query<Entity, With<Car>>,
    traffic: Query<Entity, With<Traffic>>,
) {
    // clock + speed
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: 26.0,
            ..default()
        },
        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.92)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(14.0),
            left: Val::Px(18.0),
            ..default()
        },
        HudClock,
    ));
    // speedometer
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(34.0),
            left: Val::Percent(50.0),
            margin: UiRect::left(Val::Px(-110.0)),
            width: Val::Px(220.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|p| {
            p.spawn((
                Text::new(""),
                TextFont {
                    font_size: 64.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                HudSpeed,
            ));
            p.spawn((
                Text::new(""),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(Color::srgba(1.0, 1.0, 1.0, 0.65)),
                HudSpeedUnit,
            ));
            p.spawn((
                Node {
                    width: Val::Px(200.0),
                    height: Val::Px(5.0),
                    margin: UiRect::top(Val::Px(6.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.18)),
            ))
            .with_children(|b| {
                b.spawn((
                    Node {
                        width: Val::Percent(0.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.3, 0.85, 1.0)),
                    HudBar,
                ));
            });
        });
    // prompt
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: 28.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Percent(62.0),
            left: Val::Percent(50.0),
            margin: UiRect::left(Val::Px(-200.0)),
            width: Val::Px(400.0),
            justify_content: JustifyContent::Center,
            ..default()
        },
        HudPrompt,
    ));
    // crosshair
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Percent(50.0),
            left: Val::Percent(50.0),
            margin: UiRect::new(Val::Px(-2.0), Val::Px(0.0), Val::Px(-2.0), Val::Px(0.0)),
            width: Val::Px(4.0),
            height: Val::Px(4.0),
            ..default()
        },
        BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.7)),
        BorderRadius::MAX,
        Crosshair,
    ));
    // help
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: 14.0,
            ..default()
        },
        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.6)),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(10.0),
            left: Val::Px(16.0),
            ..default()
        },
        HudHelp,
    ));

    // minimap
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(16.0),
                bottom: Val::Px(16.0),
                width: Val::Px(MAP),
                height: Val::Px(MAP),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.07, 0.1, 0.82)),
            BorderRadius::all(Val::Px(10.0)),
        ))
        .with_children(|p| {
            for c in &blocks.0 {
                p.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        width: Val::Px(BLOCK * MAP_SCALE),
                        height: Val::Px(BLOCK * MAP_SCALE),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.26, 0.31, 0.28)),
                    MapBlock(*c),
                ));
            }
            for e in &cars {
                p.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        width: Val::Px(6.0),
                        height: Val::Px(6.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(1.0, 0.85, 0.1)),
                    MapDot(e),
                ));
            }
            for e in &traffic {
                p.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        width: Val::Px(4.0),
                        height: Val::Px(4.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.85, 0.3, 0.3)),
                    MapDot(e),
                ));
            }
            p.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(MAP / 2.0 - 5.0),
                    top: Val::Px(MAP / 2.0 - 5.0),
                    width: Val::Px(10.0),
                    height: Val::Px(10.0),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.3, 0.8, 1.0)),
                BorderRadius::MAX,
                MapPlayer,
            ));
        });
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn hud(
    clock: Res<Clock>,
    driving: Res<Driving>,
    rig: Res<CamRig>,
    prompt: Res<Prompt>,
    veh: Query<&Vehicle>,
    mut clock_t: Single<&mut Text, (With<HudClock>, Without<HudSpeed>, Without<HudSpeedUnit>, Without<HudPrompt>, Without<HudHelp>)>,
    mut speed_t: Single<&mut Text, (With<HudSpeed>, Without<HudClock>, Without<HudSpeedUnit>, Without<HudPrompt>, Without<HudHelp>)>,
    mut unit_t: Single<&mut Text, (With<HudSpeedUnit>, Without<HudClock>, Without<HudSpeed>, Without<HudPrompt>, Without<HudHelp>)>,
    mut prompt_t: Single<&mut Text, (With<HudPrompt>, Without<HudClock>, Without<HudSpeed>, Without<HudSpeedUnit>, Without<HudHelp>)>,
    mut help_t: Single<&mut Text, (With<HudHelp>, Without<HudClock>, Without<HudSpeed>, Without<HudSpeedUnit>, Without<HudPrompt>)>,
    mut bar: Single<&mut Node, With<HudBar>>,
    mut cross: Single<&mut Visibility, With<Crosshair>>,
) {
    let h = clock.hours as i32;
    let m = (clock.hours.fract() * 60.0) as i32;
    clock_t.0 = format!("{h:02}:{m:02}");
    let speed = driving.0.and_then(|e| veh.get(e).ok()).map(|v| v.speed);
    match speed {
        Some(s) => {
            let kmh = (s.abs() * 3.6).round() as i32;
            speed_t.0 = format!("{kmh}");
            unit_t.0 = if s < -0.5 { "km/h  R".into() } else { "km/h".into() };
            bar.width = Val::Percent((kmh as f32 / 160.0 * 100.0).clamp(0.0, 100.0));
        }
        None => {
            speed_t.0 = String::new();
            unit_t.0 = String::new();
            bar.width = Val::Percent(0.0);
        }
    }
    prompt_t.0 = prompt.0.clone().unwrap_or_default();
    help_t.0 = if driving.0.is_some() {
        "W/S gas+brake  A/D steer  Space handbrake  H horn  F exit  V view  1-4 time  T speed".into()
    } else {
        "WASD move  Shift run  Space jump  V view  F car  Esc mouse  1-4 time  T speed".into()
    };
    let show = rig.first_person && driving.0.is_none();
    **cross = if show { Visibility::Inherited } else { Visibility::Hidden };
}

#[allow(clippy::type_complexity)]
pub fn minimap(
    driving: Res<Driving>,
    tfs: Query<&Transform, Without<MapPlayer>>,
    player: Single<&Transform, With<Player>>,
    mut blocks: Query<(&MapBlock, &mut Node), Without<MapDot>>,
    mut dots: Query<(&MapDot, &mut Node), Without<MapBlock>>,
) {
    let center = driving
        .0
        .and_then(|e| tfs.get(e).ok())
        .unwrap_or(*player)
        .translation;
    let to_px = |x: f32, z: f32| {
        (
            MAP / 2.0 + (x - center.x) * MAP_SCALE,
            MAP / 2.0 + (z - center.z) * MAP_SCALE,
        )
    };
    for (b, mut node) in &mut blocks {
        let (x, y) = to_px(b.0.x, b.0.y);
        node.left = Val::Px(x - BLOCK * MAP_SCALE / 2.0);
        node.top = Val::Px(y - BLOCK * MAP_SCALE / 2.0);
    }
    for (d, mut node) in &mut dots {
        if let Ok(t) = tfs.get(d.0) {
            let (x, y) = to_px(t.translation.x, t.translation.z);
            node.left = Val::Px(x - 3.0);
            node.top = Val::Px(y - 3.0);
        }
    }
}
