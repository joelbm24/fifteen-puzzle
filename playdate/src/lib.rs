#![no_std]

extern crate alloc;

extern crate playdate as pd;

use alloc::string::ToString;
use core::ptr::NonNull;

use fifteen::{Board, Move};
use pd::controls::buttons::PDButtonsExt;
use pd::controls::peripherals::Buttons;
use pd::display::Display;
use pd::graphics::BitmapDrawMode;
use pd::graphics::Graphics;
use pd::graphics::bitmap::Bitmap;
use pd::graphics::color::Color;
use pd::graphics::text::{self, Font};
use pd::graphics::{pop_context, push_context};
use pd::sys::EventLoopCtrl;
use pd::sys::ffi::{LCD_COLUMNS, LCD_ROWS, PlaydateAPI};
use pd::system::menu::{OptionsMenuItem, SimpleMenuItem};
use pd::system::prelude::*;
use rand::SeedableRng;
use rand::rngs::SmallRng;

#[derive(Clone, Copy, PartialEq, Eq)]
enum InputStyle {
    Cursor,
    Direct,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Screen {
    Title,
    Playing,
}

const GRID_SIZE: i32 = 4;
const TILE: i32 = 50;
const GAP: i32 = 4;
const BOARD_PIXELS: i32 = GRID_SIZE * TILE + (GRID_SIZE - 1) * GAP;

const BOLD_FONT_PATH: &str = "/System/Fonts/Asheville-Sans-14-Bold.pft";

struct State {
    screen: Screen,
    board: Board,
    bold_font: Option<Font>,
    input_style_item: Option<OptionsMenuItem>,
    new_game_item: Option<SimpleMenuItem<bool>>,
    cursor: usize,
}

impl State {
    fn new() -> Self {
        let bold_font = text::load_font(BOLD_FONT_PATH).ok();
        let input_style_item = OptionsMenuItem::new("Input", ["Direct", "Cursor"], None, ()).ok();
        if let Some(item) = &input_style_item {
            item.set_value(1);
        }
        let new_game_item = SimpleMenuItem::new("New Game", Some(|requested: &mut bool| *requested = true), false).ok();
        Self {
            screen: Screen::Title,
            board: Board::new(),
            bold_font,
            input_style_item,
            new_game_item,
            cursor: 0,
        }
    }

    fn input_style(&self) -> InputStyle {
        match self.input_style_item.as_ref().map(|item| item.selected_option()) {
            Some(1) => InputStyle::Cursor,
            _ => InputStyle::Direct,
        }
    }
}

/// Seeds a `SmallRng` from the wall-clock time, since Playdate has no OS
/// entropy source for `rand::rng()`.
fn clock_seed() -> u64 {
    let (secs, ms) = System::Default().seconds_since_epoch_with_ms();
    ((secs as u64) << 32) | (ms as u64)
}

fn reshuffle(board: &mut Board) {
    let mut rng = SmallRng::seed_from_u64(clock_seed());
    *board = Board::shuffled(&mut rng);
}

fn handle_input(state: &mut State) {
    if let Some(item) = &state.new_game_item {
        if let Some(requested) = item.get_userdata() {
            if *requested {
                *requested = false;
                reshuffle(&mut state.board);
                state.cursor = 0;
                state.screen = Screen::Playing;
                return;
            }
        }
    }

    match state.screen {
        Screen::Title => {
            if Buttons::Cached().get().pushed.a() {
                reshuffle(&mut state.board);
                state.cursor = 0;
                state.screen = Screen::Playing;
            }
        }
        Screen::Playing => {
            if state.board.is_solved() {
                if Buttons::Cached().get().pushed.a() {
                    reshuffle(&mut state.board);
                }
                return;
            }

            match state.input_style() {
                InputStyle::Direct => handle_arrow_input(&mut state.board),
                InputStyle::Cursor => handle_cursor_input(&mut state.board, &mut state.cursor),
            }
        }
    }
}

fn handle_arrow_input(board: &mut Board) {
    let buttons = Buttons::Cached().get();

    let mv = if buttons.pushed.up() {
        Some(Move::Down)
    } else if buttons.pushed.down() {
        Some(Move::Up)
    } else if buttons.pushed.left() {
        Some(Move::Right)
    } else if buttons.pushed.right() {
        Some(Move::Left)
    } else {
        None
    };

    if let Some(mv) = mv {
        let _ = board.apply_move(mv);
    }
}

fn handle_cursor_input(board: &mut Board, cursor: &mut usize) {
    let buttons = Buttons::Cached().get();

    let (mut row, mut col) = (*cursor / GRID_SIZE as usize, *cursor % GRID_SIZE as usize);

    if buttons.pushed.up() && row > 0 {
        row -= 1;
    } else if buttons.pushed.down() && row < GRID_SIZE as usize - 1 {
        row += 1;
    } else if buttons.pushed.left() && col > 0 {
        col -= 1;
    } else if buttons.pushed.right() && col < GRID_SIZE as usize - 1 {
        col += 1;
    }

    *cursor = row * GRID_SIZE as usize + col;

    if buttons.pushed.a() {
        let _ = board.slide_toward(*cursor);
    }
}

impl Update for State {
    fn update(&mut self) -> UpdateCtrl {
        handle_input(self);

        let gfx = Graphics::Default();

        gfx.clear(Color::WHITE);

        if let Some(font) = &self.bold_font {
            gfx.set_font(font);
        }

        match self.screen {
            Screen::Title => draw_title_screen(&gfx, self.bold_font.as_ref()),
            Screen::Playing => draw_playing_screen(&gfx, self),
        }

        UpdateCtrl::Continue
    }
}

fn draw_playing_screen(gfx: &Graphics, state: &State) {
    let origin_x = (LCD_COLUMNS as i32 - BOARD_PIXELS) / 2;
    let origin_y = (LCD_ROWS as i32 - BOARD_PIXELS) / 2;

    const GRID_PADDING: i32 = 8;
    const GRID_BORDER: i32 = 2;
    for i in 1..=GRID_BORDER {
        gfx.draw_rect(
            origin_x - GRID_PADDING - i,
            origin_y - GRID_PADDING - i,
            BOARD_PIXELS + (GRID_PADDING + i) * 2,
            BOARD_PIXELS + (GRID_PADDING + i) * 2,
            Color::BLACK.into(),
        );
    }

    for (index, &value) in state.board.tiles().iter().enumerate() {
        if value == 0 {
            continue;
        }

        let row = (index / GRID_SIZE as usize) as i32;
        let col = (index % GRID_SIZE as usize) as i32;
        let x = origin_x + col * (TILE + GAP);
        let y = origin_y + row * (TILE + GAP);

        let is_odd = value % 2 == 1;
        if is_odd {
            gfx.fill_rect(x, y, TILE, TILE, Color::BLACK.into());
        }
        gfx.draw_rect(x, y, TILE, TILE, Color::BLACK.into());

        let label = value.to_string();
        let text_width = gfx
            .get_text_width(&label, state.bold_font.as_ref(), 0)
            .unwrap_or(TILE / 2);
        let text_x = x + (TILE - text_width) / 2;
        let text_y = y + TILE / 2 - 8;

        if is_odd {
            let _ = gfx.set_draw_mode(BitmapDrawMode::kDrawModeInverted);
        }
        let _ = gfx.draw_text(label, text_x, text_y);
        if is_odd {
            let _ = gfx.set_draw_mode(BitmapDrawMode::kDrawModeCopy);
        }
    }

    if state.input_style() == InputStyle::Cursor {
        let row = (state.cursor / GRID_SIZE as usize) as i32;
        let col = (state.cursor % GRID_SIZE as usize) as i32;
        let x = origin_x + col * (TILE + GAP);
        let y = origin_y + row * (TILE + GAP);

        for i in 1..=3 {
            gfx.draw_rect(x - i, y - i, TILE + i * 2, TILE + i * 2, Color::BLACK.into());
        }

        gfx.draw_rect(x + 1, y + 1, TILE - 2, TILE - 2, Color::WHITE.into());
    }

    if state.board.is_solved() {
        draw_solved_banner(gfx, state.bold_font.as_ref());
    }
}

/// Small 2x2 preview of numbered tiles, used as a mini logo above the title.
fn draw_mini_logo(gfx: &Graphics, font: Option<&Font>, top_y: i32) -> i32 {
    const MINI_TILE: i32 = 28;
    const MINI_GAP: i32 = 4;

    let board_w = MINI_TILE * 2 + MINI_GAP;
    let board_h = MINI_TILE * 2 + MINI_GAP;
    let origin_x = (LCD_COLUMNS as i32 - board_w) / 2;

    for (i, &value) in [1u8, 2, 3, 4].iter().enumerate() {
        let row = (i / 2) as i32;
        let col = (i % 2) as i32;
        let x = origin_x + col * (MINI_TILE + MINI_GAP);
        let y = top_y + row * (MINI_TILE + MINI_GAP);

        let is_odd = value % 2 == 1;
        if is_odd {
            gfx.fill_rect(x, y, MINI_TILE, MINI_TILE, Color::BLACK.into());
        }
        gfx.draw_rect(x, y, MINI_TILE, MINI_TILE, Color::BLACK.into());

        let label = value.to_string();
        let text_width = gfx.get_text_width(&label, font, 0).unwrap_or(MINI_TILE / 2);
        let text_x = x + (MINI_TILE - text_width) / 2;
        let text_y = y + MINI_TILE / 2 - 8;

        if is_odd {
            let _ = gfx.set_draw_mode(BitmapDrawMode::kDrawModeInverted);
        }
        let _ = gfx.draw_text(label, text_x, text_y);
        if is_odd {
            let _ = gfx.set_draw_mode(BitmapDrawMode::kDrawModeCopy);
        }
    }

    board_h
}

/// Renders `text` small then scales it up, since the Playdate API has no
/// built-in font scaling. Returns the scaled line height.
fn draw_scaled_line_centered(gfx: &Graphics, font: Option<&Font>, text: &str, scale: i32, y: i32) -> i32 {
    let text_w = gfx.get_text_width(text, font, 0).unwrap_or(60).max(1);
    let font_h = font.map(|f| gfx.get_font_height(f) as i32).unwrap_or(16).max(1);
    let scaled_w = text_w * scale;
    let scaled_h = font_h * scale;

    let bmp_result: Result<Bitmap, _> = Bitmap::new(text_w, font_h, Color::WHITE);
    if let Ok(bmp) = bmp_result {
        push_context(&bmp);
        if let Some(font) = font {
            gfx.set_font(font);
        }
        let _ = gfx.draw_text(text, 0, 0);
        pop_context();

        let x = (LCD_COLUMNS as i32 - scaled_w) / 2;
        bmp.draw_scaled(x, y, scale as _, scale as _);
    }

    scaled_h
}

fn draw_start_hint(gfx: &Graphics, font: Option<&Font>, y: i32) {
    const CHIP_DIAMETER: i32 = 20;
    const GROUP_GAP: i32 = 8;

    let font_h = font.map(|f| gfx.get_font_height(f) as i32).unwrap_or(16).max(1);
    let hint_label = "Press A to start";
    let hint_text_w = gfx.get_text_width(hint_label, font, 0).unwrap_or(120);
    let hint_row_w = CHIP_DIAMETER + GROUP_GAP + hint_text_w;
    let hint_x = (LCD_COLUMNS as i32 - hint_row_w) / 2;

    gfx.fill_ellipse(hint_x, y, CHIP_DIAMETER, CHIP_DIAMETER, 0., 360., Color::BLACK.into());

    let a_w = gfx.get_text_width("A", font, 0).unwrap_or(6);
    let a_x = hint_x + (CHIP_DIAMETER - a_w) / 2;
    let a_y = y + (CHIP_DIAMETER - font_h) / 2;

    let _ = gfx.set_draw_mode(BitmapDrawMode::kDrawModeInverted);
    let _ = gfx.draw_text("A", a_x, a_y);
    let _ = gfx.set_draw_mode(BitmapDrawMode::kDrawModeCopy);

    let text_x = hint_x + CHIP_DIAMETER + GROUP_GAP;
    let text_y = y + (CHIP_DIAMETER - font_h) / 2;
    let _ = gfx.draw_text(hint_label, text_x, text_y);
}

fn draw_title_screen(gfx: &Graphics, font: Option<&Font>) {
    const TITLE_SCALE: i32 = 2;
    const TOP_MARGIN: i32 = 20;

    let logo_h = draw_mini_logo(gfx, font, TOP_MARGIN);

    let title_y = TOP_MARGIN + logo_h + 14;
    let title_h = draw_scaled_line_centered(gfx, font, "FIFTEEN PUZZLE", TITLE_SCALE, title_y);

    draw_start_hint(gfx, font, title_y + title_h + 20);
    draw_credit_line(gfx, font);
}

fn draw_credit_line(gfx: &Graphics, font: Option<&Font>) {
    let label = "By Joel Buchheim-Moore";
    let text_w = gfx.get_text_width(label, font, 0).unwrap_or(150);
    let font_h = font.map(|f| gfx.get_font_height(f) as i32).unwrap_or(16);

    let x = (LCD_COLUMNS as i32 - text_w) / 2;
    let y = LCD_ROWS as i32 - font_h - 14;
    let _ = gfx.draw_text(label, x, y);
}

fn draw_solved_banner(gfx: &Graphics, font: Option<&Font>) {
    const SCALE: i32 = 3;
    const CHIP_DIAMETER: i32 = 20;
    const GROUP_GAP: i32 = 8;
    const PADDING: i32 = 10;

    let label = "SOLVED!";
    let text_w = gfx.get_text_width(label, font, 0).unwrap_or(60).max(1);
    let font_h = font.map(|f| gfx.get_font_height(f) as i32).unwrap_or(16).max(1);
    let scaled_w = text_w * SCALE;
    let scaled_h = font_h * SCALE;

    let hint_label = "New Game";
    let hint_text_w = gfx.get_text_width(hint_label, font, 0).unwrap_or(70);
    let hint_row_w = CHIP_DIAMETER + GROUP_GAP + hint_text_w;
    let hint_row_h = CHIP_DIAMETER.max(font_h);

    let banner_h = PADDING * 3 + scaled_h + hint_row_h;
    let band_y = (LCD_ROWS as i32 - banner_h) / 2;

    gfx.fill_rect(0, band_y, LCD_COLUMNS as i32, banner_h, Color::BLACK.into());

    let bmp_result: Result<Bitmap, _> = Bitmap::new(text_w, font_h, Color::BLACK);
    if let Ok(bmp) = bmp_result {
        push_context(&bmp);
        if let Some(font) = font {
            gfx.set_font(font);
        }
        let _ = gfx.set_draw_mode(BitmapDrawMode::kDrawModeInverted);
        let _ = gfx.draw_text(label, 0, 0);
        let _ = gfx.set_draw_mode(BitmapDrawMode::kDrawModeCopy);
        pop_context();

        let x = (LCD_COLUMNS as i32 - scaled_w) / 2;
        let y = band_y + PADDING;
        bmp.draw_scaled(x, y, SCALE as _, SCALE as _);
    }

    let hint_y = band_y + PADDING * 2 + scaled_h;
    let hint_x = (LCD_COLUMNS as i32 - hint_row_w) / 2;

    gfx.fill_ellipse(
        hint_x,
        hint_y,
        CHIP_DIAMETER,
        CHIP_DIAMETER,
        0.,
        360.,
        Color::WHITE.into(),
    );

    let a_w = gfx.get_text_width("A", font, 0).unwrap_or(6);
    let a_x = hint_x + (CHIP_DIAMETER - a_w) / 2;
    let a_y = hint_y + (CHIP_DIAMETER - font_h) / 2;
    let _ = gfx.draw_text("A", a_x, a_y);

    let text_x = hint_x + CHIP_DIAMETER + GROUP_GAP;
    let text_y = hint_y + (CHIP_DIAMETER - font_h) / 2;
    let _ = gfx.set_draw_mode(BitmapDrawMode::kDrawModeInverted);
    let _ = gfx.draw_text(hint_label, text_x, text_y);
    let _ = gfx.set_draw_mode(BitmapDrawMode::kDrawModeCopy);
}

#[unsafe(no_mangle)]
pub fn event_handler(_: NonNull<PlaydateAPI>, event: SystemEvent, _: u32) -> EventLoopCtrl {
    if let SystemEvent::Init = event {
        Display::Default().set_refresh_rate(30.);
        System::Default().set_update_callback_boxed(|state: &mut State| state.update(), State::new());
    }

    EventLoopCtrl::Continue
}
