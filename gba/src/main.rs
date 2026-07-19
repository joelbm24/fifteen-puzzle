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
    "gfx/tile_1.png",
    "gfx/tile_2.png",
    "gfx/tile_3.png",
    "gfx/tile_4.png",
    "gfx/tile_5.png",
    "gfx/tile_6.png",
    "gfx/tile_7.png",
    "gfx/tile_8.png",
    "gfx/tile_9.png",
    "gfx/tile_10.png",
    "gfx/tile_11.png",
    "gfx/tile_12.png",
    "gfx/tile_13.png",
    "gfx/tile_14.png",
    "gfx/tile_15.png",
    "gfx/cursor.png",
    32x32 "gfx/title_fifteen.png",
    32x32 "gfx/title_puzzle.png",
    32x32 "gfx/press_start.png",
    32x32 "gfx/solved_banner.png",
    32x32 "gfx/paused_banner.png",
    "gfx/arrow.png",
);

// A real pixel/bitmap-style font (M+ Font Project's "PixelMplus10", designed
// to render crisply at exactly 10px, unlike gfx/font.ttf which is a smooth
// DejaVu Sans that looked jagged/blotchy once rasterized this small) used
// here directly (via agb's built-in text renderer) for the dynamic
// "By Joel Buchheim-Moore" credit line so it doesn't need its own pre-baked
// image asset.
static CREDIT_FONT: Font = agb::include_font!("gfx/pixel_mplus10.ttf", 10);

static CREDIT_PALETTE: &Palette16 = {
    let mut palette = [Rgb15::BLACK; 16];
    palette[1] = Rgb15::WHITE;
    &Palette16::new(palette)
};

fn tile_sprite(value: u8) -> &'static Sprite {
    match value {
        1 => sprites::TILE_1.sprite(0),
        2 => sprites::TILE_2.sprite(0),
        3 => sprites::TILE_3.sprite(0),
        4 => sprites::TILE_4.sprite(0),
        5 => sprites::TILE_5.sprite(0),
        6 => sprites::TILE_6.sprite(0),
        7 => sprites::TILE_7.sprite(0),
        8 => sprites::TILE_8.sprite(0),
        9 => sprites::TILE_9.sprite(0),
        10 => sprites::TILE_10.sprite(0),
        11 => sprites::TILE_11.sprite(0),
        12 => sprites::TILE_12.sprite(0),
        13 => sprites::TILE_13.sprite(0),
        14 => sprites::TILE_14.sprite(0),
        15 => sprites::TILE_15.sprite(0),
        _ => unreachable!("board values are always 0 (blank, handled separately) or 1..=15"),
    }
}

const fn rgb15(r: u16, g: u16, b: u16) -> Rgb15 {
    Rgb15((b << 10) | (g << 5) | r)
}

const BACKDROP_GREY: Rgb15 = rgb15(6, 6, 6);

const GRID_SIZE: i32 = 4;
const TILE: i32 = 32;
const GAP: i32 = 4;
const BOARD_PIXELS: i32 = GRID_SIZE * TILE + (GRID_SIZE - 1) * GAP;

fn reshuffle(board: &mut Board, seed: u64) {
    let mut rng = SmallRng::seed_from_u64(seed);
    *board = Board::shuffled(&mut rng);
}

/// Draws the "By Joel Buchheim-Moore" credit line, horizontally centered,
/// with its top edge at `y`.
fn show_credit_line(frame: &mut GraphicsFrame<'_>, objects: &mut Vec<Object>, y: i32) {
    // Size must be strictly larger than the font's rendered glyph bounding
    // box (ascenders/descenders included), not just the nominal font size -
    // agb's own doc examples pair an 8px font with a 16x16 sprite size for
    // exactly this reason; S8x8 panics with "y too big for sprite size".
    // PixelMplus10's ascent+descent at 10px comes to ~11px, comfortably
    // inside 16x16.
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
                    // Solved - block cursor movement and Start (no pause
                    // menu to open, there's nothing to pause), only A (new
                    // game) does anything, same rule as the Playdate port.
                    if input.is_just_pressed(Button::A) {
                        reshuffle(&mut board, frame_count);
                    }
                } else if input.is_just_pressed(Button::Start) {
                    // Open the pause menu rather than reshuffling directly -
                    // New Game/Close live there now instead.
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
                        let _ = board.slide_toward(cursor); // ignore if not aligned with blank
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
