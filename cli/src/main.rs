use std::io::{self, Stdout};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseButton, MouseEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph},
};

use fifteen::{Board, Move};

const TILE_WIDTH: u16 = 9;
const TILE_HEIGHT: u16 = 5; // 1 border + 3 content lines + 1 border — number is padded to the middle line
const GRID_SIZE: u16 = 4;

// Roughly matches the GUI's tile colors.
const ODD_TILE_COLOR: Color = Color::Rgb(0x00, 0x47, 0xAB);
const EVEN_TILE_COLOR: Color = Color::Rgb(0xCC, 0x55, 0x00);

fn main() -> io::Result<()> {
    let mut terminal = init_terminal()?;
    let result = run(&mut terminal);
    restore_terminal()?;
    result
}

fn init_terminal() -> io::Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;

    // Make sure raw mode / alt screen get cleaned up even on panic.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = restore_terminal();
        default_hook(info);
    }));

    Terminal::new(CrosstermBackend::new(io::stdout()))
}

fn restore_terminal() -> io::Result<()> {
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)
}

fn run(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> io::Result<()> {
    let mut rng = rand::rng();
    let mut board = Board::shuffled(&mut rng);

    loop {
        terminal.draw(|frame| render_board(frame, frame.area(), &board))?;

        if board.is_solved() {
            match wait_for_quit_or_new_game()? {
                PostWinAction::Quit => break,
                PostWinAction::NewGame => {
                    board = Board::shuffled(&mut rng);
                    continue;
                }
            }
        }

        match event::read()? {
            Event::Key(key) => {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                // Arrow key = direction the tile slides, so it's the opposite
                // of the direction the blank moves.
                let mv = match key.code {
                    KeyCode::Up => Some(Move::Down),
                    KeyCode::Down => Some(Move::Up),
                    KeyCode::Left => Some(Move::Right),
                    KeyCode::Right => Some(Move::Left),
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Char('n') => {
                        board = Board::shuffled(&mut rng);
                        continue;
                    }
                    _ => None,
                };

                if let Some(mv) = mv {
                    let _ = board.apply_move(mv); // ignore illegal moves at the edges
                }
            }
            Event::Mouse(mouse) => {
                if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
                    let size = terminal.size()?;
                    let area = Rect::new(0, 0, size.width, size.height);
                    if let Some(index) = tile_at(area, mouse.column, mouse.row) {
                        let _ = board.slide_toward(index); // ignore if not aligned with blank
                    }
                }
            }
            _ => {}
        }
    }

    Ok(())
}

enum PostWinAction {
    Quit,
    NewGame,
}

fn wait_for_quit_or_new_game() -> io::Result<PostWinAction> {
    loop {
        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(PostWinAction::Quit),
                KeyCode::Char('n') => return Ok(PostWinAction::NewGame),
                _ => {}
            }
        }
    }
}

fn render_board(frame: &mut Frame, area: Rect, board: &Board) {
    let (status_area, board_container) = split_status_and_board(area);

    let status = if board.is_solved() {
        "Solved! — n for new game, q to quit"
    } else {
        "Arrows or click to move — n for new game, q to quit"
    };
    frame.render_widget(Paragraph::new(status).alignment(Alignment::Center), status_area);

    let board_area = centered_board_rect(board_container);

    let outer = Block::default().title("Fifteen Puzzle").borders(Borders::ALL);
    let inner = outer.inner(board_area);
    frame.render_widget(outer, board_area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(TILE_HEIGHT); 4])
        .split(inner);

    for (row, row_area) in rows.iter().enumerate() {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(TILE_WIDTH); 4])
            .split(*row_area);

        for (col, cell_area) in cols.iter().enumerate() {
            let tile = board.tiles()[row * 4 + col];
            render_cell(frame, *cell_area, tile);
        }
    }
}

fn render_cell(frame: &mut Frame, area: Rect, tile: u8) {
    let block = Block::default().borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let style = match tile {
        0 => Style::default(),
        t if t % 2 == 1 => Style::default().bg(ODD_TILE_COLOR).fg(Color::White),
        _ => Style::default().bg(EVEN_TILE_COLOR).fg(Color::White),
    };

    // Pad the number with blank lines so it lands on the middle line
    // regardless of how tall the inner area is.
    let content_height = inner.height as usize;
    let pad_before = content_height.saturating_sub(1) / 2;
    let pad_after = content_height.saturating_sub(1).saturating_sub(pad_before);

    let label = if tile == 0 { String::new() } else { tile.to_string() };

    let mut lines = Vec::with_capacity(content_height);
    lines.extend(std::iter::repeat_with(|| Line::raw("")).take(pad_before));
    lines.push(Line::raw(label));
    lines.extend(std::iter::repeat_with(|| Line::raw("")).take(pad_after));

    let text = Paragraph::new(lines).style(style).alignment(Alignment::Center);
    frame.render_widget(text, inner);
}

fn split_status_and_board(area: Rect) -> (Rect, Rect) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(area);
    (sections[0], sections[1])
}

fn centered_board_rect(area: Rect) -> Rect {
    let board_width = TILE_WIDTH * GRID_SIZE + 2; // +2 for outer border
    let board_height = TILE_HEIGHT * GRID_SIZE + 2;
    centered_rect(board_width, board_height, area)
}

fn tile_at(area: Rect, column: u16, row: u16) -> Option<usize> {
    let (_, board_container) = split_status_and_board(area);
    let board_area = centered_board_rect(board_container);
    let inner = Block::default().borders(Borders::ALL).inner(board_area);

    if column < inner.x || row < inner.y {
        return None;
    }

    let rel_col = column - inner.x;
    let rel_row = row - inner.y;

    if rel_col >= inner.width || rel_row >= inner.height {
        return None;
    }

    let tile_col = (rel_col / TILE_WIDTH) as usize;
    let tile_row = (rel_row / TILE_HEIGHT) as usize;

    if tile_col >= GRID_SIZE as usize || tile_row >= GRID_SIZE as usize {
        return None;
    }

    Some(tile_row * GRID_SIZE as usize + tile_col)
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(area.height.saturating_sub(height) / 2),
            Constraint::Length(height),
            Constraint::Min(0),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(area.width.saturating_sub(width) / 2),
            Constraint::Length(width),
            Constraint::Min(0),
        ])
        .split(vertical[1])[1]
}
