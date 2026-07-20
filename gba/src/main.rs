#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec::Vec;

use agb::Gba;
use agb::display::GraphicsFrame;
use agb::display::Palette16;
use agb::display::Rgb15;
use agb::display::font::{AlignmentKind, Font, Layout, LayoutSettings, ObjectTextRenderer};
use agb::display::object::{Object, Size, Sprite};
use agb::input::{Button, ButtonController};

use fifteen::Board;
use rand::SeedableRng;
use rand::rngs::SmallRng;

agb::include_aseprite!(
    mod sprites,
    32x32 "gfx/tiles.png",
    "gfx/cursor.png",
    32x32 "gfx/title_fifteen.png",
    32x32 "gfx/title_puzzle.png",
    32x32 "gfx/press_start.png",
    32x32 "gfx/solved_banner.png",
    32x32 "gfx/paused_banner.png",
    "gfx/arrow.png",
);

static CREDIT_FONT: Font = agb::include_font!("gfx/pixel_mplus10.ttf", 10);

static CREDIT_PALETTE: &Palette16 = {
    let mut palette = [Rgb15::BLACK; 16];
    palette[1] = Rgb15::WHITE;
    &Palette16::new(palette)
};

fn tile_sprite(value: u8) -> &'static Sprite {
    debug_assert!(
        (1..=15).contains(&value),
        "board values are always 0 (blank, handled separately) or 1..=15"
    );
    sprites::TILES.sprite((value - 1) as usize)
}

const fn rgb15(r: u16, g: u16, b: u16) -> Rgb15 {
    Rgb15((b << 10) | (g << 5) | r)
}

const BACKDROP_GREY: Rgb15 = rgb15(2, 2, 2);

const GRID_SIZE: i32 = 4;
const TILE: i32 = 32;
const GAP: i32 = 2;
const BOARD_PIXELS: i32 = GRID_SIZE * TILE + (GRID_SIZE - 1) * GAP;

fn reshuffle(board: &mut Board, seed: u64) {
    let mut rng = SmallRng::seed_from_u64(seed);
    *board = Board::shuffled(&mut rng);
}

fn show_credit_line(frame: &mut GraphicsFrame<'_>, objects: &mut Vec<Object>, y: i32) {
    let renderer = ObjectTextRenderer::new(CREDIT_PALETTE.into(), Size::S16x16);
    let layout = Layout::new(
        "By Joel Buchheim-Moore",
        &CREDIT_FONT,
        &LayoutSettings::new()
            .with_max_line_length(agb::display::WIDTH as i32)
            .with_alignment(AlignmentKind::Centre),
    );

    for letter_group in layout {
        let obj = renderer.show(&letter_group, (0, y));
        obj.show(frame);
        objects.push(obj);
    }
}

fn show_grid(
    frame: &mut GraphicsFrame<'_>,
    objects: &mut Vec<Object>,
    sprite_at: impl Fn(usize) -> &'static Sprite,
    cols: usize,
    rows: usize,
    x: i32,
    y: i32,
) {
    for row in 0..rows {
        for col in 0..cols {
            let mut obj = Object::new(sprite_at(row * cols + col));
            obj.set_pos((x + col as i32 * TILE, y + row as i32 * TILE));
            obj.show(frame);
            objects.push(obj);
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Screen {
    Title,
    Playing,
    Paused,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PauseOption {
    NewGame,
    Close,
}

impl PauseOption {
    fn toggled(self) -> Self {
        match self {
            PauseOption::NewGame => PauseOption::Close,
            PauseOption::Close => PauseOption::NewGame,
        }
    }
}

#[agb::entry]
fn main(mut gba: Gba) -> ! {
    let mut gfx = gba.graphics.get();
    let mut input = ButtonController::new();

    gfx.set_background_palette_colour_256(0, BACKDROP_GREY);

    let origin_x = (agb::display::WIDTH as i32 - BOARD_PIXELS) / 2;
    let origin_y = (agb::display::HEIGHT as i32 - BOARD_PIXELS) / 2;

    let mut board = Board::new(); // starts solved
    let mut frame_count: u64 = 0;
    let mut cursor: usize = 0;
    let mut screen = Screen::Title;
    let mut pause_option = PauseOption::NewGame;

    loop {
        input.update();
        frame_count = frame_count.wrapping_add(1);

        match screen {
            Screen::Title => {
                if input.is_just_pressed(Button::Start) {
                    reshuffle(&mut board, frame_count);
                    cursor = 0;
                    screen = Screen::Playing;
                }
            }
            Screen::Playing => {
                if board.is_solved() {
                    if input.is_just_pressed(Button::A) {
                        reshuffle(&mut board, frame_count);
                    }
                } else if input.is_just_pressed(Button::Start) {
                    pause_option = PauseOption::NewGame;
                    screen = Screen::Paused;
                } else {
                    let (mut row, mut col) = (cursor / GRID_SIZE as usize, cursor % GRID_SIZE as usize);

                    if input.is_just_pressed(Button::Up) && row > 0 {
                        row -= 1;
                    } else if input.is_just_pressed(Button::Down) && row < GRID_SIZE as usize - 1 {
                        row += 1;
                    } else if input.is_just_pressed(Button::Left) && col > 0 {
                        col -= 1;
                    } else if input.is_just_pressed(Button::Right) && col < GRID_SIZE as usize - 1 {
                        col += 1;
                    }

                    cursor = row * GRID_SIZE as usize + col;

                    if input.is_just_pressed(Button::A) {
                        let _ = board.slide_toward(cursor);
                    }
                }
            }
            Screen::Paused => {
                if input.is_just_pressed(Button::Up) || input.is_just_pressed(Button::Down) {
                    pause_option = pause_option.toggled();
                }

                if input.is_just_pressed(Button::B) || input.is_just_pressed(Button::Start) {
                    screen = Screen::Playing; // resume, unchanged
                } else if input.is_just_pressed(Button::A) {
                    match pause_option {
                        PauseOption::NewGame => {
                            reshuffle(&mut board, frame_count);
                            cursor = 0;
                            screen = Screen::Playing;
                        }
                        PauseOption::Close => screen = Screen::Playing,
                    }
                }
            }
        }

        let mut frame = gfx.frame();
        let mut objects: Vec<Object> = Vec::new();

        match screen {
            Screen::Title => {
                const LOGO_GAP: i32 = 2;
                const LOGO_X: i32 = 10;
                for (i, &value) in [1u8, 2, 3, 4].iter().enumerate() {
                    let row = (i / 2) as i32;
                    let col = (i % 2) as i32;
                    let mut obj = Object::new(tile_sprite(value));
                    obj.set_pos((LOGO_X + col * (TILE + LOGO_GAP), 24 + row * (TILE + LOGO_GAP)));
                    obj.show(&mut frame);
                    objects.push(obj);
                }

                show_grid(
                    &mut frame,
                    &mut objects,
                    |i| sprites::TITLE_FIFTEEN.sprite(i),
                    4,
                    1,
                    80,
                    24,
                );
                show_grid(
                    &mut frame,
                    &mut objects,
                    |i| sprites::TITLE_PUZZLE.sprite(i),
                    5,
                    1,
                    80,
                    60,
                );

                show_grid(
                    &mut frame,
                    &mut objects,
                    |i| sprites::PRESS_START.sprite(i),
                    4,
                    1,
                    56,
                    104,
                );

                show_credit_line(&mut frame, &mut objects, 140);
            }
            Screen::Playing | Screen::Paused => {
                if screen == Screen::Playing && board.is_solved() {
                    show_grid(
                        &mut frame,
                        &mut objects,
                        |i| sprites::SOLVED_BANNER.sprite(i),
                        5,
                        2,
                        40,
                        48,
                    );
                }

                if screen == Screen::Paused {
                    let arrow_y = match pause_option {
                        PauseOption::NewGame => 32 + 32 + 8,
                        PauseOption::Close => 32 + 64 + 8,
                    };
                    let mut arrow_obj = Object::new(sprites::ARROW.sprite(0));
                    arrow_obj.set_pos((60, arrow_y));
                    arrow_obj.show(&mut frame);
                    objects.push(arrow_obj);

                    show_grid(
                        &mut frame,
                        &mut objects,
                        |i| sprites::PAUSED_BANNER.sprite(i),
                        5,
                        3,
                        40,
                        32,
                    );
                }

                let cursor_row = (cursor / GRID_SIZE as usize) as i32;
                let cursor_col = (cursor % GRID_SIZE as usize) as i32;
                let cursor_x = origin_x + cursor_col * (TILE + GAP);
                let cursor_y = origin_y + cursor_row * (TILE + GAP);
                let mut cursor_obj = Object::new(sprites::CURSOR.sprite(0));
                cursor_obj.set_pos((cursor_x, cursor_y));
                cursor_obj.show(&mut frame);
                objects.push(cursor_obj);

                for (index, &value) in board.tiles().iter().enumerate() {
                    if value == 0 {
                        continue;
                    }

                    let row = (index / GRID_SIZE as usize) as i32;
                    let col = (index % GRID_SIZE as usize) as i32;
                    let x = origin_x + col * (TILE + GAP);
                    let y = origin_y + row * (TILE + GAP);

                    let mut tile_obj = Object::new(tile_sprite(value));
                    tile_obj.set_pos((x, y));
                    tile_obj.show(&mut frame);
                    objects.push(tile_obj);
                }
            }
        }

        frame.commit();
    }
}
