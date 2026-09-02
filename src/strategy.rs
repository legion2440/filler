use std::cmp::Ordering;
use std::collections::{BTreeSet, VecDeque};

use crate::{generate_placements, Board, Piece, Player, Position};

const INFINITE_DISTANCE: i32 = i32::MAX / 4;
const DEEP_EVALUATION: usize = 72;
const CARDINAL_DIRECTIONS: [Position; 4] = [
    Position { x: 1, y: 0 },
    Position { x: -1, y: 0 },
    Position { x: 0, y: 1 },
    Position { x: 0, y: -1 },
];

#[derive(Default)]
pub struct Strategy;

#[derive(Clone, Copy)]
struct ScoredMove {
    position: Position,
    score: i64,
}

impl Strategy {
    pub fn new() -> Self {
        Self
    }

    pub fn choose(&self, board: &Board, piece: &Piece, player: Player) -> Option<Position> {
        let placements = generate_placements(board, piece, player);
        if placements.is_empty() {
            return None;
        }
        if placements.len() == 1 {
            return Some(placements[0]);
        }

        let enemy_distance = distance_field(board, |cell| player.is_enemy(cell), None);
        let contact_distance = closest_territory_distance(board, player, &enemy_distance);

        let mut moves = Vec::with_capacity(placements.len());
        for position in placements {
            moves.push(ScoredMove {
                position,
                score: quick_score(
                    board,
                    piece,
                    player,
                    position,
                    &enemy_distance,
                    contact_distance,
                ),
            });
        }

        moves.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then_with(|| a.position.y.cmp(&b.position.y))
                .then_with(|| a.position.x.cmp(&b.position.x))
        });

        let limit = moves.len().min(DEEP_EVALUATION);
        let mut best = ScoredMove {
            position: moves[0].position,
            score: i64::MIN / 4,
        };

        for candidate in moves.iter().take(limit) {
            let position = candidate.position;
            let deep = territory_score(board, piece, player, position, &enemy_distance);
            let frontier = projected_frontier(board, piece, player, position, true);
            let enemy_frontier = projected_frontier(board, piece, player, position, false);
            let combined =
                candidate.score + deep * 3 + frontier as i64 * 7 - enemy_frontier as i64 * 5;

            if combined > best.score || (combined == best.score && prefer(position, best.position))
            {
                best = ScoredMove {
                    position,
                    score: combined,
                };
            }
        }

        Some(best.position)
    }
}

fn quick_score(
    board: &Board,
    piece: &Piece,
    player: Player,
    position: Position,
    enemy_distance: &[Vec<i32>],
    contact_distance: i32,
) -> i64 {
    let mut min_enemy = INFINITE_DISTANCE;
    let mut sum_enemy = 0_i64;
    let mut new_cells = 0_i64;
    let mut frontier = BTreeSet::new();
    let mut blocking = 0_i64;
    let mut edge_cells = 0_i64;

    for piece_cell in &piece.cells {
        let x = position.x + piece_cell.x;
        let y = position.y + piece_cell.y;
        if player.is_own(board.at(x, y)) {
            continue;
        }

        new_cells += 1;
        let distance = enemy_distance[y as usize][x as usize];
        min_enemy = min_enemy.min(distance);
        if distance < INFINITE_DISTANCE {
            sum_enemy += distance as i64;
        }
        if distance <= 2 {
            blocking += 1;
        }
        if x == 0 || y == 0 || x as usize == board.width - 1 || y as usize == board.height - 1 {
            edge_cells += 1;
        }

        for direction in CARDINAL_DIRECTIONS {
            let next = Position {
                x: x + direction.x,
                y: y + direction.y,
            };
            if next.x < 0
                || next.y < 0
                || next.x as usize >= board.width
                || next.y as usize >= board.height
            {
                continue;
            }
            if board.at(next.x, next.y) == b'.' && !piece_occupies(piece, position, next) {
                frontier.insert(next);
            }
        }
    }

    if min_enemy == INFINITE_DISTANCE {
        min_enemy = (board.width + board.height) as i32;
    }

    let (attack_weight, territory_weight) = if contact_distance <= 4 {
        (72_i64, 14_i64)
    } else {
        (115_i64, 8_i64)
    };

    let center_x = board.width as i32 / 2;
    let center_y = board.height as i32 / 2;
    let anchor_x = position.x + piece.width as i32 / 2;
    let anchor_y = position.y + piece.height as i32 / 2;
    let center_distance = (anchor_x - center_x).abs() + (anchor_y - center_y).abs();

    -(min_enemy as i64) * attack_weight - sum_enemy * 3
        + blocking * 46
        + frontier.len() as i64 * territory_weight
        + new_cells * 5
        - edge_cells * 3
        - center_distance as i64
}

fn territory_score(
    board: &Board,
    piece: &Piece,
    player: Player,
    position: Position,
    enemy_distance: &[Vec<i32>],
) -> i64 {
    let projected: BTreeSet<Position> = piece
        .cells
        .iter()
        .map(|cell| Position {
            x: position.x + cell.x,
            y: position.y + cell.y,
        })
        .collect();

    let own_distance = distance_field(board, |cell| player.is_own(cell), Some(&projected));

    let mut score = 0_i64;
    for y in 0..board.height {
        for x in 0..board.width {
            if board.rows[y][x] != b'.' {
                continue;
            }
            let point = Position {
                x: x as i32,
                y: y as i32,
            };
            if projected.contains(&point) {
                continue;
            }

            match own_distance[y][x].cmp(&enemy_distance[y][x]) {
                Ordering::Less => score += 1,
                Ordering::Greater => score -= 1,
                Ordering::Equal => {}
            }
        }
    }
    score
}

fn distance_field<F>(board: &Board, seed: F, extra: Option<&BTreeSet<Position>>) -> Vec<Vec<i32>>
where
    F: Fn(u8) -> bool,
{
    let mut distance = vec![vec![INFINITE_DISTANCE; board.width]; board.height];
    let mut queue = VecDeque::with_capacity(board.width * board.height);

    for (y, (board_row, distance_row)) in board.rows.iter().zip(distance.iter_mut()).enumerate() {
        for (x, (&cell, slot)) in board_row.iter().zip(distance_row.iter_mut()).enumerate() {
            let point = Position {
                x: x as i32,
                y: y as i32,
            };
            let is_extra = extra.map_or(false, |points| points.contains(&point));
            if seed(cell) || is_extra {
                *slot = 0;
                queue.push_back(point);
            }
        }
    }

    while let Some(current) = queue.pop_front() {
        let current_distance = distance[current.y as usize][current.x as usize];
        for direction in CARDINAL_DIRECTIONS {
            let next = Position {
                x: current.x + direction.x,
                y: current.y + direction.y,
            };
            if next.x < 0
                || next.y < 0
                || next.x as usize >= board.width
                || next.y as usize >= board.height
            {
                continue;
            }
            let slot = &mut distance[next.y as usize][next.x as usize];
            if *slot <= current_distance + 1 {
                continue;
            }
            *slot = current_distance + 1;
            queue.push_back(next);
        }
    }

    distance
}

fn projected_frontier(
    board: &Board,
    piece: &Piece,
    player: Player,
    position: Position,
    own: bool,
) -> usize {
    let projected: BTreeSet<Position> = piece
        .cells
        .iter()
        .map(|cell| Position {
            x: position.x + cell.x,
            y: position.y + cell.y,
        })
        .collect();

    let mut frontier = BTreeSet::new();
    for y in 0..board.height {
        for x in 0..board.width {
            let point = Position {
                x: x as i32,
                y: y as i32,
            };
            let mut is_territory = player.is_own(board.rows[y][x]);
            if own {
                if projected.contains(&point) {
                    is_territory = true;
                }
            } else {
                is_territory = player.is_enemy(board.rows[y][x]);
            }
            if !is_territory {
                continue;
            }

            for direction in CARDINAL_DIRECTIONS {
                let next = Position {
                    x: point.x + direction.x,
                    y: point.y + direction.y,
                };
                if next.x < 0
                    || next.y < 0
                    || next.x as usize >= board.width
                    || next.y as usize >= board.height
                {
                    continue;
                }
                if board.at(next.x, next.y) != b'.' || projected.contains(&next) {
                    continue;
                }
                frontier.insert(next);
            }
        }
    }
    frontier.len()
}

fn closest_territory_distance(board: &Board, player: Player, enemy_distance: &[Vec<i32>]) -> i32 {
    let mut best = INFINITE_DISTANCE;
    for (board_row, distance_row) in board.rows.iter().zip(enemy_distance.iter()) {
        for (&cell, &distance) in board_row.iter().zip(distance_row.iter()) {
            if player.is_own(cell) {
                best = best.min(distance);
            }
        }
    }
    best
}

fn piece_occupies(piece: &Piece, position: Position, point: Position) -> bool {
    piece
        .cells
        .iter()
        .any(|cell| position.x + cell.x == point.x && position.y + cell.y == point.y)
}

fn prefer(candidate: Position, current: Position) -> bool {
    candidate.y < current.y || (candidate.y == current.y && candidate.x < current.x)
}
