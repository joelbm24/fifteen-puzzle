#![cfg_attr(not(test), no_std)]

extern crate alloc;

use alloc::vec::Vec;
use core::fmt;
use rand::Rng;
use rand::seq::SliceRandom;
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
        let tiles: [u8; 16] = core::array::from_fn(|i| if i < 15 { (i + 1) as u8 } else { 0 });
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
            return Ok(());
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
            return Err(MoveError::IllegalMove);
        };

        while self.blank != target {
            self.apply_move(mv)?;
        }

        Ok(())
    }

    /// Produces a uniformly random *solvable* shuffled board, using the
    /// caller-supplied RNG (kept generic so `no_std` targets can pass their
    /// own source instead of `rand::rng()`).
    pub fn shuffled(rng: &mut impl Rng) -> Self {
        let mut tiles: [u8; 16] = core::array::from_fn(|i| if i < 15 { (i + 1) as u8 } else { 0 });
        tiles.shuffle(rng);

        let blank = tiles.iter().position(|&t| t == 0).expect("exactly one blank");
        let mut board = Board { tiles, blank };

        if !board.is_solvable() {
            let mut non_blank_indices = board.tiles.iter().enumerate().filter(|&(_, &t)| t != 0).map(|(i, _)| i);
            let i = non_blank_indices.next().expect("15 non-blank tiles");
            let j = non_blank_indices.next().expect("15 non-blank tiles");
            board.tiles.swap(i, j);
        }

        board
    }

    /// A 15-puzzle arrangement is solvable iff permutation parity matches the
    /// blank's taxicab distance parity from its solved position.
    pub fn is_solvable(&self) -> bool {
        let mut visited = [false; 16];
        let mut cycles = 0;
        for start in 0..16 {
            if visited[start] {
                continue;
            }
            cycles += 1;
            let mut i = start;
            while !visited[i] {
                visited[i] = true;
                i = (self.tiles[i] as usize + 15) % 16;
            }
        }
        let permutation_is_odd = (16 - cycles) % 2 == 1;

        const SOLVED_BLANK: usize = 15;
        let (row, col) = (self.blank / 4, self.blank % 4);
        let (solved_row, solved_col) = (SOLVED_BLANK / 4, SOLVED_BLANK % 4);
        let taxicab_distance = row.abs_diff(solved_row) + col.abs_diff(solved_col);
        let distance_is_odd = taxicab_distance % 2 == 1;

        permutation_is_odd == distance_is_odd
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solved_board_is_solvable() {
        assert!(Board::new().is_solvable());
    }

    #[test]
    fn classic_14_15_swap_is_unsolvable() {
        let mut board = Board::new();
        board.tiles.swap(13, 14);
        assert!(!board.is_solvable());
    }

    #[test]
    fn shuffled_is_always_solvable() {
        let mut rng = rand::rng();
        for _ in 0..1000 {
            assert!(Board::shuffled(&mut rng).is_solvable());
        }
    }

    #[test]
    fn solvable_after_legal_moves_with_blank_elsewhere() {
        let mut board = Board::new();
        board.apply_move(Move::Up).unwrap();
        board.apply_move(Move::Left).unwrap();
        assert!(board.is_solvable());
    }
}
