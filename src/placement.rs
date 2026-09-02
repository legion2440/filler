use std::collections::BTreeSet;

use crate::{Board, Piece, Player, Position};

pub fn validate_placement(
    board: &Board,
    piece: &Piece,
    player: Player,
    position: Position,
) -> bool {
    let mut overlaps = 0;

    for cell in &piece.cells {
        let x = position.x + cell.x;
        let y = position.y + cell.y;
        if x < 0 || y < 0 || x as usize >= board.width || y as usize >= board.height {
            return false;
        }

        let board_cell = board.at(x, y);
        if player.is_enemy(board_cell) {
            return false;
        }
        if player.is_own(board_cell) {
            overlaps += 1;
            if overlaps > 1 {
                return false;
            }
        } else if board_cell != b'.' {
            return false;
        }
    }

    overlaps == 1
}

pub fn generate_placements(board: &Board, piece: &Piece, player: Player) -> Vec<Position> {
    if piece.cells.is_empty() {
        return Vec::new();
    }

    let mut seen = BTreeSet::new();
    let mut placements = Vec::new();

    for y in 0..board.height {
        for x in 0..board.width {
            if !player.is_own(board.at(x as i32, y as i32)) {
                continue;
            }

            for piece_cell in &piece.cells {
                let candidate = Position {
                    x: x as i32 - piece_cell.x,
                    y: y as i32 - piece_cell.y,
                };
                if !seen.insert(candidate) {
                    continue;
                }
                if validate_placement(board, piece, player, candidate) {
                    placements.push(candidate);
                }
            }
        }
    }

    placements
}
