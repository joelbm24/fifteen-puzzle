#![no_main]
#![no_std]

extern crate flipperzero_rt;

extern crate flipperzero_alloc;

use fifteen::{Board, Move};

use core::convert::Infallible;
use core::ffi::{CStr, c_char, c_void};

use flipperzero::furi::message_queue::MessageQueue;
use flipperzero::furi::rng::HwRng;
use flipperzero::furi::sync::Mutex;
use flipperzero::furi::time::FuriDuration;
use flipperzero::gui::canvas::Align;
use flipperzero_rt::{entry, manifest};
use flipperzero_sys as sys;
use flipperzero_sys::Color;
use flipperzero_sys::furi::UnsafeRecord;

use rand::TryRng;
use rand_core::RngCore as _;

use embedded_graphics::mono_font::{MonoTextStyle, ascii::FONT_6X10};
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{
    Circle, PrimitiveStyle, PrimitiveStyleBuilder, Rectangle, StrokeAlignment, Triangle,
};
use embedded_graphics::text::{Alignment, Text};

const FULLSCREEN: sys::GuiLayer = sys::GuiLayerFullscreen;

manifest!(name = "Fifteen Puzzle", has_icon = true, icon = "puzzle-10x10.icon");
entry!(main);

const TILE_SIZE: i32 = 15;
const TILE_GAP: i32 = 1;
const GRID_OFFSET: i32 = 30;
const GRID_SIZE: i32 = 4;
const CURSOR_THICKNESS: i32 = 2;
const MENU_ITEM_COUNT: usize = 3;
const MENU_ROW_HEIGHT: i32 = 14;
const MENU_HEADER_HEIGHT: i32 = 12;
const MENU_SELECTION_INSET: i32 = 2;

/// `Direct` moves the blank directly with the arrow keys; `Cursor` moves a
/// highlighted selector and slides whatever tile it's on toward the blank.
#[derive(Clone, Copy, PartialEq, Eq)]
enum InputStyle {
    Direct,
    Cursor,
}

/// Bridges `HwRng` (implements `rand_core 0.6`'s `RngCore`) to the `rand_core
/// 0.10` `Rng` trait `fifteen::Board::shuffled` expects.
struct FlipperRng(HwRng);

impl FlipperRng {
    fn new() -> Self {
        FlipperRng(HwRng)
    }
}

impl TryRng for FlipperRng {
    type Error = Infallible;

    fn try_next_u32(&mut self) -> Result<u32, Infallible> {
        Ok(self.0.next_u32())
    }

    fn try_next_u64(&mut self) -> Result<u64, Infallible> {
        Ok(self.0.next_u64())
    }

    fn try_fill_bytes(&mut self, dst: &mut [u8]) -> Result<(), Infallible> {
        self.0.fill_bytes(dst);
        Ok(())
    }
}

/// Behind one lock so a move, a cursor step, and a menu action can never be
/// applied against a half-updated view of the others.
struct Game {
    board: Board,
    cursor: usize,
    input_style: InputStyle,
    menu_open: bool,
    menu_selection: usize,
}

/// Shared between `draw_callback` and `input_callback`, which the GUI service
/// invokes from its own thread, separate from the one `main` runs on.
struct State {
    game: Mutex<Game>,
    queue: MessageQueue<sys::InputEvent>,
}

fn reshuffle(game: &mut Game) {
    game.board = Board::shuffled(&mut FlipperRng::new());
    game.cursor = 0;
}

/// Activates whichever menu row is highlighted. Toggling input style leaves
/// the menu open; the other two rows close it.
fn activate_menu_selection(game: &mut Game) {
    match game.menu_selection {
        0 => {
            game.input_style = match game.input_style {
                InputStyle::Direct => InputStyle::Cursor,
                InputStyle::Cursor => InputStyle::Direct,
            };
        }
        1 => {
            reshuffle(game);
            game.menu_open = false;
        }
        _ => game.menu_open = false,
    }
}

fn u8_to_cstr(n: u8, buf: &mut [u8; 4]) -> &CStr {
    let mut i = 3;
    buf[3] = 0;
    let mut n = n;
    loop {
        i -= 1;
        buf[i] = b'0' + (n % 10);
        n /= 10;
        if n == 0 {
            break;
        }
    }

    let len = 4 - i;
    buf.copy_within(i..4, 0);
    CStr::from_bytes_with_nul(&buf[..len]).unwrap()
}

/// Redraws the board each frame, then layers the cursor, solved banner, and
/// menu overlays on top of it (each one mutually exclusive with the others).
pub unsafe extern "C" fn draw_callback(canvas: *mut sys::Canvas, context: *mut c_void) {
    unsafe {
        let state = &*(context as *const State);
        let game = state.game.lock();

        sys::canvas_clear(canvas);

        for (idx, &num) in game.board.tiles().iter().enumerate() {
            let row = (idx / GRID_SIZE as usize) as i32;
            let col = (idx % GRID_SIZE as usize) as i32;

            if num == 0 {
                continue;
            } else if num % 2 == 0 {
                sys::canvas_set_color(canvas, sys::ColorBlack);
                sys::canvas_draw_frame(
                    canvas,
                    GRID_OFFSET + (col * TILE_SIZE) + (TILE_GAP * col),
                    (row * TILE_SIZE) + (TILE_GAP * row),
                    TILE_SIZE as usize,
                    TILE_SIZE as usize,
                );
                sys::canvas_set_color(canvas, sys::ColorBlack);
            } else {
                sys::canvas_set_color(canvas, sys::ColorBlack);
                sys::canvas_draw_box(
                    canvas,
                    GRID_OFFSET + (col * TILE_SIZE) + (TILE_GAP * col),
                    (row * TILE_SIZE) + (TILE_GAP * row),
                    TILE_SIZE as usize,
                    TILE_SIZE as usize,
                );
                sys::canvas_set_color(canvas, sys::ColorWhite);
            }

            let mut num_buf = [0u8; 4];
            let cstr = u8_to_cstr(num, &mut num_buf);
            let p: *const c_char = cstr.as_ptr();

            sys::canvas_draw_str_aligned(
                canvas,
                GRID_OFFSET + 8 + (col * TILE_SIZE) + (TILE_GAP * col),
                (row * TILE_SIZE) + (TILE_GAP * row) + 8,
                Align::Center.to_sys(),
                Align::Center.to_sys(),
                p,
            );
        }

        if game.input_style == InputStyle::Cursor && !game.board.is_solved() && !game.menu_open {
            let cursor_row = (game.cursor / GRID_SIZE as usize) as i32;
            let cursor_col = (game.cursor % GRID_SIZE as usize) as i32;
            let x = GRID_OFFSET + (cursor_col * TILE_SIZE) + (TILE_GAP * cursor_col);
            let y = (cursor_row * TILE_SIZE) + (TILE_GAP * cursor_row);

            let tile_under_cursor = game.board.tiles()[game.cursor];
            let cursor_over_black_tile = tile_under_cursor != 0 && tile_under_cursor % 2 == 1;
            sys::canvas_set_color(
                canvas,
                if cursor_over_black_tile {
                    sys::ColorWhite
                } else {
                    sys::ColorBlack
                },
            );

            for i in 0..CURSOR_THICKNESS {
                let inset = 1 + i;
                sys::canvas_draw_frame(
                    canvas,
                    x + inset,
                    y + inset,
                    (TILE_SIZE - 2 * inset) as usize,
                    (TILE_SIZE - 2 * inset) as usize,
                );
            }
        }

        if game.board.is_solved() && !game.menu_open {
            let canvas_w = sys::canvas_width(canvas) as i32;
            let canvas_h = sys::canvas_height(canvas) as i32;

            const BANNER_HEIGHT: i32 = 24;
            let banner_y = (canvas_h - BANNER_HEIGHT) / 2;
            let center_x = canvas_w / 2;

            sys::canvas_set_color(canvas, sys::ColorBlack);
            sys::canvas_draw_box(canvas, 0, banner_y, canvas_w as usize, BANNER_HEIGHT as usize);

            sys::canvas_set_color(canvas, sys::ColorWhite);
            sys::canvas_draw_str_aligned(
                canvas,
                center_x,
                banner_y + 8,
                Align::Center.to_sys(),
                Align::Center.to_sys(),
                c"SOLVED!".as_ptr(),
            );
            sys::canvas_draw_str_aligned(
                canvas,
                center_x,
                banner_y + 18,
                Align::Center.to_sys(),
                Align::Center.to_sys(),
                c"Ok: New Game".as_ptr(),
            );
        }

        if game.menu_open {
            let canvas_w = sys::canvas_width(canvas) as i32;
            let canvas_h = sys::canvas_height(canvas) as i32;

            let menu_height = MENU_HEADER_HEIGHT + MENU_ROW_HEIGHT * MENU_ITEM_COUNT as i32;
            let menu_y = (canvas_h - menu_height) / 2;
            let center_x = canvas_w / 2;

            sys::canvas_set_color(canvas, sys::ColorBlack);
            sys::canvas_draw_box(canvas, 0, menu_y, canvas_w as usize, menu_height as usize);

            sys::canvas_set_color(canvas, sys::ColorWhite);
            sys::canvas_draw_str_aligned(
                canvas,
                center_x,
                menu_y + MENU_HEADER_HEIGHT / 2,
                Align::Center.to_sys(),
                Align::Center.to_sys(),
                c"Menu".as_ptr(),
            );
            sys::canvas_draw_line(
                canvas,
                0,
                menu_y + MENU_HEADER_HEIGHT,
                canvas_w,
                menu_y + MENU_HEADER_HEIGHT,
            );

            let input_label: &CStr = match game.input_style {
                InputStyle::Direct => c"Input: Direct",
                InputStyle::Cursor => c"Input: Cursor",
            };
            let labels: [&CStr; MENU_ITEM_COUNT] = [input_label, c"New Game", c"Close"];

            for (i, label) in labels.iter().enumerate() {
                let row_y = menu_y + MENU_HEADER_HEIGHT + i as i32 * MENU_ROW_HEIGHT;

                if i == game.menu_selection {
                    sys::canvas_set_color(canvas, sys::ColorWhite);
                    sys::canvas_draw_box(
                        canvas,
                        0,
                        row_y + MENU_SELECTION_INSET,
                        canvas_w as usize,
                        (MENU_ROW_HEIGHT - 2 * MENU_SELECTION_INSET) as usize,
                    );
                    sys::canvas_set_color(canvas, sys::ColorBlack);
                } else {
                    sys::canvas_set_color(canvas, sys::ColorWhite);
                }

                sys::canvas_draw_str_aligned(
                    canvas,
                    center_x,
                    row_y + MENU_ROW_HEIGHT / 2,
                    Align::Center.to_sys(),
                    Align::Center.to_sys(),
                    label.as_ptr(),
                );
            }
        }
    }
}

/// Pushes each event onto the queue; drops it if full rather than blocking
/// the GUI thread this runs on.
pub unsafe extern "C" fn input_callback(input_event: *mut sys::InputEvent, context: *mut c_void) {
    unsafe {
        let state = &*(context as *const State);
        let _ = state.queue.put(*input_event, FuriDuration::from_millis(0));
    }
}

/// Input priority per event: Back, then the menu (if open), then long-press
/// Ok to open it, then the solved-state shortcut, then normal board input.
fn main(_args: Option<&CStr>) -> i32 {
    let state = State {
        game: Mutex::new(Game {
            board: Board::shuffled(&mut FlipperRng::new()),
            cursor: 0,
            input_style: InputStyle::Cursor,
            menu_open: false,
            menu_selection: 0,
        }),
        queue: MessageQueue::new(8),
    };

    unsafe {
        let view_port = sys::view_port_alloc();
        sys::view_port_draw_callback_set(view_port, Some(draw_callback), &state as *const _ as *mut c_void);
        sys::view_port_input_callback_set(view_port, Some(input_callback), &state as *const _ as *mut c_void);

        {
            let gui = UnsafeRecord::open(c"gui");
            sys::gui_add_view_port(gui.as_ptr(), view_port, FULLSCREEN);

            'game: loop {
                if let Ok(event) = state.queue.get(FuriDuration::WAIT_FOREVER) {
                    let mut should_exit = false;
                    let mut needs_redraw = false;

                    {
                        let mut game = state.game.lock();

                        if event.key == sys::InputKeyBack && event.type_ == sys::InputTypePress {
                            if game.menu_open {
                                game.menu_open = false;
                                needs_redraw = true;
                            } else {
                                should_exit = true;
                            }
                        } else if game.menu_open {
                            if event.type_ == sys::InputTypePress {
                                match event.key {
                                    sys::InputKeyUp => {
                                        game.menu_selection =
                                            (game.menu_selection + MENU_ITEM_COUNT - 1) % MENU_ITEM_COUNT;
                                        needs_redraw = true;
                                    }
                                    sys::InputKeyDown => {
                                        game.menu_selection = (game.menu_selection + 1) % MENU_ITEM_COUNT;
                                        needs_redraw = true;
                                    }
                                    _ => {}
                                }
                            } else if event.key == sys::InputKeyOk && event.type_ == sys::InputTypeShort {
                                activate_menu_selection(&mut game);
                                needs_redraw = true;
                            }
                        } else if event.key == sys::InputKeyOk && event.type_ == sys::InputTypeLong {
                            game.menu_open = true;
                            game.menu_selection = 0;
                            needs_redraw = true;
                        } else if game.board.is_solved() {
                            if event.key == sys::InputKeyOk && event.type_ == sys::InputTypeShort {
                                reshuffle(&mut game);
                                needs_redraw = true;
                            }
                        } else if event.key == sys::InputKeyOk && event.type_ == sys::InputTypeShort {
                            if game.input_style == InputStyle::Cursor {
                                let cursor = game.cursor;
                                let _ = game.board.slide_toward(cursor);
                                needs_redraw = true;
                            }
                        } else if event.type_ == sys::InputTypePress {
                            match game.input_style {
                                InputStyle::Direct => {
                                    let mv = match event.key {
                                        sys::InputKeyUp => Some(Move::Down),
                                        sys::InputKeyDown => Some(Move::Up),
                                        sys::InputKeyLeft => Some(Move::Right),
                                        sys::InputKeyRight => Some(Move::Left),
                                        _ => None,
                                    };
                                    if let Some(mv) = mv {
                                        let _ = game.board.apply_move(mv);
                                        needs_redraw = true;
                                    }
                                }
                                InputStyle::Cursor => {
                                    let (mut row, mut col) =
                                        (game.cursor / GRID_SIZE as usize, game.cursor % GRID_SIZE as usize);

                                    let moved = match event.key {
                                        sys::InputKeyUp if row > 0 => {
                                            row -= 1;
                                            true
                                        }
                                        sys::InputKeyDown if row < GRID_SIZE as usize - 1 => {
                                            row += 1;
                                            true
                                        }
                                        sys::InputKeyLeft if col > 0 => {
                                            col -= 1;
                                            true
                                        }
                                        sys::InputKeyRight if col < GRID_SIZE as usize - 1 => {
                                            col += 1;
                                            true
                                        }
                                        _ => false,
                                    };

                                    if moved {
                                        game.cursor = row * GRID_SIZE as usize + col;
                                        needs_redraw = true;
                                    }
                                }
                            }
                        }
                    }

                    if should_exit {
                        break 'game;
                    }
                    if needs_redraw {
                        sys::view_port_update(view_port);
                    }
                }
            }

            sys::view_port_enabled_set(view_port, false);
            sys::gui_remove_view_port(gui.as_ptr(), view_port);
        }
        sys::view_port_free(view_port);
    }

    0
}
