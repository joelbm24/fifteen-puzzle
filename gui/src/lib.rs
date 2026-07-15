use bevy::input_focus::{FocusCause, InputFocus};
use bevy::prelude::*;
use fifteen::{Board, Move};

const fn from_hex(hex: u32) -> Color {
    let r = ((hex >> 16) & 0xFF) as u8 as f32 / 255.0;
    let g = ((hex >> 8) & 0xFF) as u8 as f32 / 255.0;
    let b = (hex & 0xFF) as u8 as f32 / 255.0;
    Color::srgb(r, g, b)
}

const GRID_SIZE: i32 = 4;
const TILE_SIZE: f32 = 100.0;
const TILE_GAP: f32 = 3.0;
const TILE_BORDER: f32 = 6.0;
const TILE_SPEED: f32 = 900.0; // pixels per second

const NORMAL_BUTTON: Color = Color::srgb(0.15, 0.15, 0.15);
const HOVERED_BUTTON: Color = Color::srgb(0.25, 0.25, 0.25);
const PRESSED_BUTTON: Color = Color::srgb(0.10, 0.10, 0.10);

const TILE_SHADOW: Color = Color::srgba(0.0, 0.0, 0.0, 0.35);
const TILE_FACE: Color = from_hex(0x0047AB);
const TILE_FACE2: Color = from_hex(0xCC5500);

#[derive(Resource)]
struct BoardState(Board);

#[derive(Resource)]
struct MenuMessage {
    text: String,
    is_win: bool,
}

impl Default for MenuMessage {
    fn default() -> Self {
        MenuMessage {
            text: "Fifteen Puzzle".to_string(),
            is_win: false,
        }
    }
}

#[derive(Resource, Default)]
struct PendingWin(bool);

#[derive(Resource, Default)]
struct HasPlayed(bool);

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
enum GameState {
    #[default]
    Menu,
    Playing,
}

#[derive(Component)]
struct Tile {
    value: u8,
}

#[derive(Component)]
struct TargetPos(Vec3);

#[derive(Component)]
struct NewGameButton;

#[derive(Component)]
struct MenuRoot;

#[derive(Component)]
struct IngameButton;

#[derive(Component)]
struct OpenConfirmButton;

#[derive(Component)]
struct CancelConfirmButton;

#[derive(Component)]
struct ConfirmMenuRoot;

pub fn run_app() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Fifteen Puzzle".to_string(),
                #[cfg(not(any(target_os = "ios", target_os = "android")))]
                resolution: bevy::window::WindowResolution::new(700, 620),
                #[cfg(any(target_os = "ios", target_os = "android"))]
                mode: bevy::window::WindowMode::BorderlessFullscreen(bevy::window::MonitorSelection::Current),
                ..default()
            }),
            ..default()
        }))
        .init_resource::<InputFocus>() // required for UI button accessibility
        .init_resource::<MenuMessage>()
        .init_resource::<PendingWin>()
        .init_resource::<HasPlayed>()
        .insert_resource(BoardState(Board::shuffled(200)))
        .init_state::<GameState>()
        .add_systems(Startup, (setup_camera, spawn_tiles, spawn_ingame_button).chain())
        .add_systems(OnEnter(GameState::Menu), (spawn_menu, hide_ingame_button))
        .add_systems(OnExit(GameState::Menu), despawn_menu)
        .add_systems(OnEnter(GameState::Playing), (show_tiles, show_ingame_button))
        .add_systems(
            Update,
            (
                handle_input.run_if(no_confirm_menu),
                handle_pointer_input.run_if(no_confirm_menu),
                sync_tiles,
                detect_win,
                animate_tiles,
                show_win_when_settled,
            )
                .chain()
                .run_if(in_state(GameState::Playing)),
        )
        .add_systems(Update, (button_system, open_confirm_menu, cancel_confirm_menu))
        .run();
}

#[unsafe(no_mangle)]
pub extern "C" fn bevy_main() {
    run_app();
}

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn no_confirm_menu(confirm_roots: Query<(), With<ConfirmMenuRoot>>) -> bool {
    confirm_roots.is_empty()
}

// ----- Start / win menu -----

fn spawn_menu(mut commands: Commands, message: Res<MenuMessage>) {
    if message.is_win {
        // Win screen: title + button sit on a darker, padded panel.
        commands.spawn((
            MenuRoot,
            Node {
                width: percent(100),
                height: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
            children![(
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    row_gap: px(20),
                    padding: UiRect::all(px(40)),
                    width: percent(90),
                    border_radius: BorderRadius::all(px(24)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.85)),
                children![
                    (
                        Text::new(message.text.clone()),
                        TextFont {
                            font_size: FontSize::Px(50.0),
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ),
                    new_game_button_bundle(),
                ],
            )],
        ));
    } else {
        // Title screen: original plain look, no panel.
        commands.spawn((
            MenuRoot,
            Node {
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: px(20),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
            children![
                (
                    Text::new(message.text.clone()),
                    TextFont {
                        font_size: FontSize::Px(50.0),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ),
                new_game_button_bundle(),
            ],
        ));
    }
}

fn despawn_menu(mut commands: Commands, roots: Query<Entity, With<MenuRoot>>) {
    for entity in &roots {
        commands.entity(entity).despawn();
    }
}

fn button_system(
    mut commands: Commands,
    mut input_focus: ResMut<InputFocus>,
    mut interaction_query: Query<
        (
            Entity,
            &Interaction,
            &mut BackgroundColor,
            &mut BorderColor,
            &mut Button,
        ),
        (Changed<Interaction>, With<NewGameButton>),
    >,
    confirm_roots: Query<Entity, With<ConfirmMenuRoot>>,
    mut board: ResMut<BoardState>,
    mut has_played: ResMut<HasPlayed>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    for (entity, interaction, mut color, mut border_color, mut button) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                input_focus.set(entity, FocusCause::Pressed);
                *color = BackgroundColor(PRESSED_BUTTON);
                *border_color = BorderColor::all(Color::BLACK);
                button.set_changed();

                if has_played.0 {
                    board.0 = Board::shuffled(200);
                }
                has_played.0 = true;

                for entity in &confirm_roots {
                    commands.entity(entity).despawn();
                }

                next_state.set(GameState::Playing);
            }
            Interaction::Hovered => {
                input_focus.set(entity, FocusCause::Pressed);
                *color = BackgroundColor(HOVERED_BUTTON);
                *border_color = BorderColor::all(Color::WHITE);
                button.set_changed();
            }
            Interaction::None => {
                input_focus.clear();
                *color = BackgroundColor(NORMAL_BUTTON);
                *border_color = BorderColor::all(Color::BLACK);
            }
        }
    }
}

// ----- Tiles -----

fn spawn_tiles(mut commands: Commands, board: Res<BoardState>) {
    for (index, &value) in board.0.tiles().iter().enumerate() {
        if value == 0 {
            continue; // blank tile has no entity
        }

        commands
            .spawn((
                Tile { value },
                Transform::from_translation(position_for_index(index)),
                TargetPos(position_for_index(index)),
                Visibility::Hidden,
            ))
            .with_children(|tile| {
                // Drop shadow, offset down-right, drawn behind everything.
                tile.spawn((
                    Sprite::from_color(TILE_SHADOW, Vec2::splat(TILE_SIZE)),
                    Transform::from_xyz(4.0, -4.0, 0.0),
                ));

                let tile_color = if value % 2 == 0 { TILE_FACE } else { TILE_FACE2 };

                // Darker frame — full tile size, shows as a border.
                // let tile_frame_color = if value % 2 == 0 { TILE_FRAME } else { TILE_FRAME2 };
                let darken = if value % 2 == 0 { 0.05 } else { 0.10 };
                let tile_frame_color: Color = tile_color.to_srgba().darker(darken).into();

                tile.spawn((
                    Sprite::from_color(tile_frame_color, Vec2::splat(TILE_SIZE)),
                    Transform::from_xyz(0.0, 0.0, 0.1),
                ));

                tile.spawn((
                    Sprite::from_color(tile_color, Vec2::splat(TILE_SIZE - TILE_BORDER * 2.0)),
                    Transform::from_xyz(0.0, 0.0, 0.2),
                ));

                // Tile number, on top.
                tile.spawn((
                    Text2d::new(value.to_string()),
                    TextFont {
                        font_size: FontSize::Px(40.0),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    Transform::from_xyz(0.0, 0.0, 0.3),
                ));
            });
    }
}

fn show_tiles(mut tiles: Query<&mut Visibility, With<Tile>>) {
    for mut visibility in &mut tiles {
        *visibility = Visibility::Visible;
    }
}

// ----- In-game corner button + confirm dialog -----

fn spawn_ingame_button(mut commands: Commands) {
    commands.spawn((
        IngameButton,
        Visibility::Hidden,
        Node {
            position_type: PositionType::Absolute,
            #[cfg(any(target_os = "ios", target_os = "android"))]
            top: px(60),
            #[cfg(not(any(target_os = "ios", target_os = "android")))]
            top: px(16),
            right: px(16),
            ..default()
        },
        children![(
            Button,
            OpenConfirmButton,
            Node {
                width: px(140),
                height: px(45),
                border: UiRect::all(px(3)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border_radius: BorderRadius::MAX,
                ..default()
            },
            BorderColor::all(Color::BLACK),
            BackgroundColor(NORMAL_BUTTON),
            children![(
                Text::new("Menu"),
                TextFont {
                    font_size: FontSize::Px(20.0),
                    ..default()
                },
                TextColor(Color::srgb(0.9, 0.9, 0.9)),
            )]
        )],
    ));
}

fn show_ingame_button(mut buttons: Query<&mut Visibility, With<IngameButton>>) {
    for mut visibility in &mut buttons {
        *visibility = Visibility::Visible;
    }
}

fn hide_ingame_button(mut buttons: Query<&mut Visibility, With<IngameButton>>) {
    for mut visibility in &mut buttons {
        *visibility = Visibility::Hidden;
    }
}

fn spawn_confirm_menu(commands: &mut Commands) {
    commands.spawn((
        ConfirmMenuRoot,
        Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
        children![(
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: px(20),
                width: percent(90),
                padding: UiRect::all(px(40)),
                border_radius: BorderRadius::all(px(24)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.9)),
            children![
                (
                    Text::new("Start a new game?"),
                    TextFont {
                        font_size: FontSize::Px(30.0),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ),
                (
                    Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: px(20),
                        ..default()
                    },
                    children![
                        (
                            Button,
                            NewGameButton,
                            Node {
                                width: px(160),
                                height: px(55),
                                border: UiRect::all(px(4)),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                border_radius: BorderRadius::MAX,
                                ..default()
                            },
                            BorderColor::all(Color::BLACK),
                            BackgroundColor(NORMAL_BUTTON),
                            children![(
                                Text::new("New Game"),
                                TextFont {
                                    font_size: FontSize::Px(24.0),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.9, 0.9, 0.9)),
                            )]
                        ),
                        (
                            Button,
                            CancelConfirmButton,
                            Node {
                                width: px(160),
                                height: px(55),
                                border: UiRect::all(px(4)),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                border_radius: BorderRadius::MAX,
                                ..default()
                            },
                            BorderColor::all(Color::BLACK),
                            BackgroundColor(NORMAL_BUTTON),
                            children![(
                                Text::new("Exit"),
                                TextFont {
                                    font_size: FontSize::Px(24.0),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.9, 0.9, 0.9)),
                            )]
                        ),
                    ],
                ),
            ],
        )],
    ));
}

fn open_confirm_menu(
    mut commands: Commands,
    mut interactions: Query<
        (&Interaction, &mut BackgroundColor, &mut BorderColor, &mut Button),
        (Changed<Interaction>, With<OpenConfirmButton>),
    >,
    existing: Query<Entity, With<ConfirmMenuRoot>>,
) {
    for (interaction, mut color, mut border_color, mut button) in &mut interactions {
        match *interaction {
            Interaction::Pressed => {
                *color = BackgroundColor(PRESSED_BUTTON);
                *border_color = BorderColor::all(Color::BLACK);
                button.set_changed();

                if existing.is_empty() {
                    spawn_confirm_menu(&mut commands);
                }
            }
            Interaction::Hovered => {
                *color = BackgroundColor(HOVERED_BUTTON);
                *border_color = BorderColor::all(Color::WHITE);
                button.set_changed();
            }
            Interaction::None => {
                *color = BackgroundColor(NORMAL_BUTTON);
                *border_color = BorderColor::all(Color::BLACK);
            }
        }
    }
}

fn cancel_confirm_menu(
    mut commands: Commands,
    mut interactions: Query<
        (&Interaction, &mut BackgroundColor, &mut BorderColor, &mut Button),
        (Changed<Interaction>, With<CancelConfirmButton>),
    >,
    roots: Query<Entity, With<ConfirmMenuRoot>>,
) {
    for (interaction, mut color, mut border_color, mut button) in &mut interactions {
        match *interaction {
            Interaction::Pressed => {
                *color = BackgroundColor(PRESSED_BUTTON);
                *border_color = BorderColor::all(Color::BLACK);
                button.set_changed();

                for entity in &roots {
                    commands.entity(entity).despawn();
                }
            }
            Interaction::Hovered => {
                *color = BackgroundColor(HOVERED_BUTTON);
                *border_color = BorderColor::all(Color::WHITE);
                button.set_changed();
            }
            Interaction::None => {
                *color = BackgroundColor(NORMAL_BUTTON);
                *border_color = BorderColor::all(Color::BLACK);
            }
        }
    }
}

// ----- Input -----

fn handle_input(keys: Res<ButtonInput<KeyCode>>, mut board: ResMut<BoardState>) {
    // Arrow key = direction the tile slides, so it's the opposite
    // of the direction the blank moves.
    let mv = if keys.just_pressed(KeyCode::ArrowUp) {
        Some(Move::Down)
    } else if keys.just_pressed(KeyCode::ArrowDown) {
        Some(Move::Up)
    } else if keys.just_pressed(KeyCode::ArrowLeft) {
        Some(Move::Right)
    } else if keys.just_pressed(KeyCode::ArrowRight) {
        Some(Move::Left)
    } else {
        None
    };

    if let Some(mv) = mv {
        let _ = board.0.apply_move(mv); // ignore illegal moves at the edges
    }
}

fn handle_pointer_input(
    mouse: Res<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
    windows: Query<&Window>,
    camera: Query<(&Camera, &GlobalTransform)>,
    mut board: ResMut<BoardState>,
) {
    let Ok(window) = windows.single() else { return };
    let Ok((camera, camera_transform)) = camera.single() else {
        return;
    };

    let screen_pos = if mouse.just_pressed(MouseButton::Left) {
        window.cursor_position()
    } else {
        touches.iter_just_pressed().next().map(|t| t.position())
    };

    let Some(screen_pos) = screen_pos else { return };
    let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, screen_pos) else {
        return;
    };

    if let Some(index) = index_for_position(world_pos) {
        let _ = board.0.slide_toward(index); // ignore clicks off-axis from the blank
    }
}

/// Inverse of `position_for_index` — maps a world-space point to the
/// nearest grid cell, or `None` if it falls outside the board.
fn index_for_position(pos: Vec2) -> Option<usize> {
    let step = TILE_SIZE + TILE_GAP;
    let offset = step * (GRID_SIZE as f32 - 1.0) / 2.0;

    let col = ((pos.x + offset) / step).round() as i32;
    let row = ((offset - pos.y) / step).round() as i32;

    if (0..GRID_SIZE).contains(&col) && (0..GRID_SIZE).contains(&row) {
        Some((row * GRID_SIZE + col) as usize)
    } else {
        None
    }
}

// ----- Sync / animation / win check -----

fn sync_tiles(board: Res<BoardState>, mut tiles: Query<(&Tile, &mut TargetPos)>) {
    if !board.is_changed() {
        return;
    }

    for (tile, mut target) in &mut tiles {
        if let Some(index) = board.0.tiles().iter().position(|&v| v == tile.value) {
            target.0 = position_for_index(index);
        }
    }
}

fn animate_tiles(time: Res<Time>, mut tiles: Query<(&TargetPos, &mut Transform)>) {
    let max_delta = TILE_SPEED * time.delta_secs();

    for (target, mut transform) in &mut tiles {
        transform.translation = move_towards(transform.translation, target.0, max_delta);
    }
}

/// Moves `current` toward `target` by at most `max_delta` units, snapping
/// exactly onto it once within range.
fn move_towards(current: Vec3, target: Vec3, max_delta: f32) -> Vec3 {
    let delta = target - current;
    let dist = delta.length();

    if dist <= max_delta || dist == 0.0 {
        target
    } else {
        current + delta / dist * max_delta
    }
}

fn detect_win(board: Res<BoardState>, mut pending_win: ResMut<PendingWin>) {
    if !board.is_changed() {
        return;
    }

    if board.0.is_solved() {
        pending_win.0 = true;
    }
}

fn show_win_when_settled(
    mut pending_win: ResMut<PendingWin>,
    tiles: Query<(&Transform, &TargetPos)>,
    mut menu_message: ResMut<MenuMessage>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if !pending_win.0 {
        return;
    }

    let all_settled = tiles
        .iter()
        .all(|(transform, target)| transform.translation.distance(target.0) < 0.5);

    if all_settled {
        pending_win.0 = false;
        menu_message.text = "You win!".to_string();
        menu_message.is_win = true;
        next_state.set(GameState::Menu);
    }
}

/// Converts a flat 0..16 board index into a centered world-space position.
fn position_for_index(index: usize) -> Vec3 {
    let row = (index / GRID_SIZE as usize) as f32;
    let col = (index % GRID_SIZE as usize) as f32;
    let step = TILE_SIZE + TILE_GAP;
    let offset = step * (GRID_SIZE as f32 - 1.0) / 2.0;

    Vec3::new(col * step - offset, offset - row * step, 0.0)
}

fn new_game_button_bundle() -> impl Bundle {
    (
        Button,
        NewGameButton,
        Node {
            width: px(200),
            height: px(65),
            border: UiRect::all(px(5)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border_radius: BorderRadius::MAX,
            ..default()
        },
        BorderColor::all(Color::BLACK),
        BackgroundColor(NORMAL_BUTTON),
        children![(
            Text::new("New Game"),
            TextFont {
                font_size: FontSize::Px(33.0),
                ..default()
            },
            TextColor(Color::srgb(0.9, 0.9, 0.9)),
        )],
    )
}
