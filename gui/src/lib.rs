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

#[derive(Resource, Clone, Copy)]
struct GridMetrics {
    tile_size: f32,
    gap: f32,
    border: f32,
}

impl GridMetrics {
    fn step(&self) -> f32 {
        self.tile_size + self.gap
    }

    fn ui_scale(&self) -> f32 {
        (self.tile_size / TILE_SIZE).max(0.5)
    }
}

fn compute_grid_metrics(window_size: Vec2) -> GridMetrics {
    let shortest_side = window_size.x.min(window_size.y);

    const MAX_TILE_SIZE: f32 = TILE_SIZE;

    const GAP_RATIO: f32 = TILE_GAP / TILE_SIZE;
    const BORDER_RATIO: f32 = TILE_BORDER / TILE_SIZE;
    const FOOTPRINT_COEFF: f32 = GRID_SIZE as f32 + (GRID_SIZE as f32 - 1.0) * GAP_RATIO;
    const SCREEN_MARGIN: f32 = 0.92; // leave a little breathing room at the edges

    let fitted_tile_size = (shortest_side * SCREEN_MARGIN) / FOOTPRINT_COEFF;
    let tile_size = fitted_tile_size.min(MAX_TILE_SIZE);

    GridMetrics {
        tile_size,
        gap: tile_size * GAP_RATIO,
        border: tile_size * BORDER_RATIO,
    }
}

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
enum GameState {
    #[default]
    Loading,
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

#[cfg(target_os = "android")]
#[bevy_main]
fn main() {
    run_app();
}

pub fn run_app() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Fifteen Puzzle".to_string(),
                #[cfg(not(any(target_os = "ios", target_os = "android")))]
                resolution: bevy::window::WindowResolution::new(700, 620),
                #[cfg(target_os = "ios")]
                mode: bevy::window::WindowMode::BorderlessFullscreen(bevy::window::MonitorSelection::Current),
                #[cfg(target_os = "android")]
                mode: bevy::window::WindowMode::BorderlessFullscreen(bevy::window::MonitorSelection::Primary),
                ..default()
            }),
            ..default()
        }))
        .init_resource::<InputFocus>() // required for UI button accessibility
        .init_resource::<MenuMessage>()
        .init_resource::<PendingWin>()
        .init_resource::<HasPlayed>()
        .insert_resource(BoardState(Board::shuffled(&mut rand::rng())))
        .init_state::<GameState>()
        .add_systems(Startup, setup_camera)
        .add_systems(Update, resolve_screen_metrics.run_if(in_state(GameState::Loading)))
        .add_systems(OnExit(GameState::Loading), spawn_ingame_button)
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
    commands.spawn((
        Camera2d,
        #[cfg(target_os = "android")]
        Msaa::Off,
    ));
}

fn no_confirm_menu(confirm_roots: Query<(), With<ConfirmMenuRoot>>) -> bool {
    confirm_roots.is_empty()
}

fn spawn_menu(mut commands: Commands, message: Res<MenuMessage>, metrics: Res<GridMetrics>) {
    let scale = metrics.ui_scale();

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
                    row_gap: px(20.0 * scale),
                    padding: UiRect::all(px(40.0 * scale)),
                    width: percent(90),
                    border_radius: BorderRadius::all(px(24.0 * scale)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.85)),
                children![
                    (
                        Text::new(message.text.clone()),
                        TextFont {
                            font_size: FontSize::Px(50.0 * scale),
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ),
                    new_game_button_bundle(scale),
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
                row_gap: px(20.0 * scale),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
            children![
                (
                    Text::new(message.text.clone()),
                    TextFont {
                        font_size: FontSize::Px(50.0 * scale),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ),
                new_game_button_bundle(scale),
                (
                    Text::new("By Joel Buchheim-Moore"),
                    TextFont {
                        font_size: FontSize::Px(18.0 * scale),
                        ..default()
                    },
                    TextColor(Color::srgba(1.0, 1.0, 1.0, 0.65)),
                ),
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
                    board.0 = Board::shuffled(&mut rand::rng());
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

fn resolve_screen_metrics(
    mut commands: Commands,
    windows: Query<&Window>,
    board: Res<BoardState>,
    mut resize_events: MessageReader<bevy::window::WindowResized>,
    mut frames_waited: Local<u32>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    const FALLBACK_FRAMES: u32 = 10;

    let size = if let Some(event) = resize_events.read().last() {
        Vec2::new(event.width, event.height)
    } else {
        *frames_waited += 1;
        if *frames_waited < FALLBACK_FRAMES {
            return; // give a real resize message a chance to arrive first
        }
        let Ok(window) = windows.single() else { return };
        Vec2::new(window.width(), window.height())
    };

    let metrics = compute_grid_metrics(size);
    spawn_tiles_with_metrics(&mut commands, &board, &metrics);
    commands.insert_resource(metrics);
    next_state.set(GameState::Menu);
}

fn spawn_tiles_with_metrics(commands: &mut Commands, board: &BoardState, metrics: &GridMetrics) {
    let shadow_offset = metrics.tile_size * 0.04;
    let font_size = metrics.tile_size * 0.4;

    for (index, &value) in board.0.tiles().iter().enumerate() {
        if value == 0 {
            continue; // blank tile has no entity
        }

        let pos = position_for_index(index, metrics);

        commands
            .spawn((
                Tile { value },
                Transform::from_translation(pos),
                TargetPos(pos),
                Visibility::Hidden,
            ))
            .with_children(|tile| {
                // Drop shadow, offset down-right, drawn behind everything.
                tile.spawn((
                    Sprite::from_color(TILE_SHADOW, Vec2::splat(metrics.tile_size)),
                    Transform::from_xyz(shadow_offset, -shadow_offset, 0.0),
                ));

                let tile_color = if value % 2 == 0 { TILE_FACE } else { TILE_FACE2 };

                // Darker frame — full tile size, shows as a border.
                let darken = if value % 2 == 0 { 0.05 } else { 0.10 };
                let tile_frame_color: Color = tile_color.to_srgba().darker(darken).into();

                tile.spawn((
                    Sprite::from_color(tile_frame_color, Vec2::splat(metrics.tile_size)),
                    Transform::from_xyz(0.0, 0.0, 0.1),
                ));

                tile.spawn((
                    Sprite::from_color(tile_color, Vec2::splat(metrics.tile_size - metrics.border * 2.0)),
                    Transform::from_xyz(0.0, 0.0, 0.2),
                ));

                // Tile number, on top.
                tile.spawn((
                    Text2d::new(value.to_string()),
                    TextFont {
                        font_size: FontSize::Px(font_size),
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

fn spawn_ingame_button(mut commands: Commands, metrics: Res<GridMetrics>) {
    let scale = metrics.ui_scale();

    commands.spawn((
        IngameButton,
        Visibility::Hidden,
        Node {
            position_type: PositionType::Absolute,
            #[cfg(any(target_os = "ios", target_os = "android"))]
            top: px(60.0 * scale),
            #[cfg(not(any(target_os = "ios", target_os = "android")))]
            top: px(16.0 * scale),
            right: px(16.0 * scale),
            ..default()
        },
        children![(
            Button,
            OpenConfirmButton,
            Node {
                width: px(140.0 * scale),
                height: px(45.0 * scale),
                border: UiRect::all(px(3.0 * scale)),
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
                    font_size: FontSize::Px(20.0 * scale),
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

fn spawn_confirm_menu(commands: &mut Commands, metrics: &GridMetrics) {
    let scale = metrics.ui_scale();

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
                row_gap: px(20.0 * scale),
                width: percent(90),
                padding: UiRect::all(px(40.0 * scale)),
                border_radius: BorderRadius::all(px(24.0 * scale)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.9)),
            children![
                (
                    Text::new("Start a new game?"),
                    TextFont {
                        font_size: FontSize::Px(30.0 * scale),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ),
                (
                    Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: px(20.0 * scale),
                        ..default()
                    },
                    children![
                        (
                            Button,
                            NewGameButton,
                            Node {
                                width: px(160.0 * scale),
                                height: px(55.0 * scale),
                                border: UiRect::all(px(4.0 * scale)),
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
                                    font_size: FontSize::Px(24.0 * scale),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.9, 0.9, 0.9)),
                            )]
                        ),
                        (
                            Button,
                            CancelConfirmButton,
                            Node {
                                width: px(160.0 * scale),
                                height: px(55.0 * scale),
                                border: UiRect::all(px(4.0 * scale)),
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
                                    font_size: FontSize::Px(24.0 * scale),
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
    metrics: Option<Res<GridMetrics>>,
) {
    let Some(metrics) = metrics else { return }; // not resolved yet (Loading)

    for (interaction, mut color, mut border_color, mut button) in &mut interactions {
        match *interaction {
            Interaction::Pressed => {
                *color = BackgroundColor(PRESSED_BUTTON);
                *border_color = BorderColor::all(Color::BLACK);
                button.set_changed();

                if existing.is_empty() {
                    spawn_confirm_menu(&mut commands, &metrics);
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
    metrics: Option<Res<GridMetrics>>,
) {
    let Some(metrics) = metrics else { return }; // tiles/metrics not ready yet
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

    if let Some(index) = index_for_position(world_pos, &metrics) {
        let _ = board.0.slide_toward(index); // ignore clicks off-axis from the blank
    }
}

/// Inverse of `position_for_index` — maps a world-space point to the
/// nearest grid cell, or `None` if it falls outside the board.
fn index_for_position(pos: Vec2, metrics: &GridMetrics) -> Option<usize> {
    let step = metrics.step();
    let offset = step * (GRID_SIZE as f32 - 1.0) / 2.0;

    let col = ((pos.x + offset) / step).round() as i32;
    let row = ((offset - pos.y) / step).round() as i32;

    if (0..GRID_SIZE).contains(&col) && (0..GRID_SIZE).contains(&row) {
        Some((row * GRID_SIZE + col) as usize)
    } else {
        None
    }
}

fn sync_tiles(board: Res<BoardState>, metrics: Option<Res<GridMetrics>>, mut tiles: Query<(&Tile, &mut TargetPos)>) {
    let Some(metrics) = metrics else { return };
    if !board.is_changed() {
        return;
    }

    for (tile, mut target) in &mut tiles {
        if let Some(index) = board.0.tiles().iter().position(|&v| v == tile.value) {
            target.0 = position_for_index(index, &metrics);
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
        menu_message.text = "Solved!".to_string();
        menu_message.is_win = true;
        next_state.set(GameState::Menu);
    }
}

/// Converts a flat 0..16 board index into a centered world-space position.
fn position_for_index(index: usize, metrics: &GridMetrics) -> Vec3 {
    let row = (index / GRID_SIZE as usize) as f32;
    let col = (index % GRID_SIZE as usize) as f32;
    let step = metrics.step();
    let offset = step * (GRID_SIZE as f32 - 1.0) / 2.0;

    Vec3::new(col * step - offset, offset - row * step, 0.0)
}

fn new_game_button_bundle(scale: f32) -> impl Bundle {
    (
        Button,
        NewGameButton,
        Node {
            width: px(200.0 * scale),
            height: px(65.0 * scale),
            border: UiRect::all(px(5.0 * scale)),
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
                font_size: FontSize::Px(33.0 * scale),
                ..default()
            },
            TextColor(Color::srgb(0.9, 0.9, 0.9)),
        )],
    )
}
