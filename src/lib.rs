use rand::RngExt;
use std::fmt;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum MoveError {
    #[error("illegal move: blank is at the edge")]
    OutOfBounds,

    #[error("illegal move: blank is at the edge")]
    IllegalMove,
}

pub struct Board {
    pub tiles: [u8; 16],
    pub blank: usize,
}

impl Board {
    pub fn new() -> Self {
        let tiles: [u8; 16] = std::array::from_fn(|i| if i < 15 { (i + 1) as u8 } else { 0 });
        Board { tiles, blank: 15 }
    }

    pub fn is_solved(&self) -> bool {
        self.tiles
            .iter()
            .enumerate()
            .all(|(i, &t)| if i == 15 { t == 0 } else { t == (i + 1) as u8 })
    }

    pub fn tiles(&self) -> &[u8; 16] {
        &self.tiles
    }

    pub fn apply_move(&mut self, mv: Move) -> Result<(), MoveError> {
        let row = self.blank / 4;
        let col = self.blank % 4;

        let target = match mv {
            Move::Up if row > 0 => self.blank - 4,
            Move::Down if row < 3 => self.blank + 4,
            Move::Left if col > 0 => self.blank - 1,
            Move::Right if col < 3 => self.blank + 1,
            _ => return Err(MoveError::OutOfBounds),
        };

        self.tiles.swap(self.blank, target);
        self.blank = target;
        Ok(())
    }

    pub fn legal_moves(&self) -> Vec<Move> {
        let row = self.blank / 4;
        let col = self.blank % 4;

        let mut moves = Vec::with_capacity(4);
        if row > 0 {
            moves.push(Move::Up);
        }
        if row < 3 {
            moves.push(Move::Down);
        }
        if col > 0 {
            moves.push(Move::Left);
        }
        if col < 3 {
            moves.push(Move::Right);
        }
        moves
    }

    pub fn slide_toward(&mut self, target: usize) -> Result<(), MoveError> {
        if target == self.blank {
            return Ok(()); // clicked the blank itself, nothing to do
        }

        let (blank_row, blank_col) = (self.blank / 4, self.blank % 4);
        let (target_row, target_col) = (target / 4, target % 4);

        let mv = if blank_row == target_row {
            if target_col < blank_col {
                Move::Left
            } else {
                Move::Right
            }
        } else if blank_col == target_col {
            if target_row < blank_row { Move::Up } else { Move::Down }
        } else {
            return Err(MoveError::IllegalMove); // not in the same row or column — illegal click
        };

        while self.blank != target {
            self.apply_move(mv)?;
        }

        Ok(())
    }

    pub fn shuffled(n: usize) -> Self {
        let mut board = Board::new();
        let mut rng = rand::rng(); // was thread_rng()
        let mut last_move: Option<Move> = None;

        for _ in 0..n {
            let candidates: Vec<Move> = board
                .legal_moves()
                .into_iter()
                .filter(|&m| last_move.map_or(true, |last| m != last.opposite()))
                .collect();

            let mv = candidates[rng.random_range(0..candidates.len())]; // was gen_range()
            board.apply_move(mv).expect("legal_moves() only returns valid moves");
            last_move = Some(mv);
        }

        board
    }
}

impl Default for Board {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for Board {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for row in self.tiles.chunks(4) {
            for &tile in row {
                if tile == 0 {
                    write!(f, " []")?;
                } else {
                    write!(f, "{:3} ", tile)?;
                }
            }
            writeln!(f)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Move {
    Up,
    Down,
    Left,
    Right,
}

impl Move {
    pub fn opposite(self) -> Move {
        match self {
            Move::Up => Move::Down,
            Move::Down => Move::Up,
            Move::Left => Move::Right,
            Move::Right => Move::Left,
        }
    }
}
