use std::collections::BTreeSet;
use std::io::Cursor;

use filler::{
    format_move, generate_placements, validate_placement, Board, Parser, Piece, Player, Position,
    Strategy,
};

fn board(rows: &[&str]) -> Board {
    Board {
        width: rows[0].len(),
        height: rows.len(),
        rows: rows.iter().map(|row| row.as_bytes().to_vec()).collect(),
    }
}

fn piece(rows: &[&str]) -> Piece {
    let mut cells = Vec::new();
    for (y, row) in rows.iter().enumerate() {
        for (x, cell) in row.bytes().enumerate() {
            if cell != b'.' {
                cells.push(Position {
                    x: x as i32,
                    y: y as i32,
                });
            }
        }
    }
    Piece {
        width: rows[0].len(),
        height: rows.len(),
        rows: rows.iter().map(|row| row.as_bytes().to_vec()).collect(),
        cells,
    }
}

#[test]
fn parses_player_board_and_piece() {
    let input = b"$$$ exec p1 : [/filler/solution/filler]\n\
Anfield 5 4:\n\
    01234\n\
000 .....\n\
001 ..@..\n\
002 .....\n\
003 ...$.\n\
Piece 3 2:\n\
.OO\n\
.O.\n";
    let mut parser = Parser::new(Cursor::new(input));
    let player = parser.read_player().unwrap();
    assert_eq!(player.number, 1);

    let turn = parser.next_turn().unwrap().unwrap();
    assert_eq!((turn.board.width, turn.board.height), (5, 4));
    assert_eq!(turn.board.rows[1], b"..@..");
    assert_eq!((turn.piece.width, turn.piece.height), (3, 2));
    assert_eq!(
        turn.piece.cells,
        vec![
            Position { x: 1, y: 0 },
            Position { x: 2, y: 0 },
            Position { x: 1, y: 1 }
        ]
    );
}

#[test]
fn parses_multiple_turns_from_one_stream() {
    let input = b"$$$ exec p1 : [/filler/solution/filler]\n\
Anfield 3 2:\n\
    012\n\
000 .@.\n\
001 .$..\n\
Piece 2 1:\n\
OO\n\
Anfield 3 2:\n\
    012\n\
000 .a.\n\
001 .s.\n\
Piece 1 2:\n\
O\n\
O\n";
    let mut parser = Parser::new(Cursor::new(input));
    let player = parser.read_player().unwrap();
    assert_eq!(player.number, 1);

    let first = parser.next_turn().unwrap().unwrap();
    assert_eq!(first.board.rows[0], b".@.");
    assert_eq!(first.piece.cells.len(), 2);

    let second = parser.next_turn().unwrap().unwrap();
    assert_eq!(second.board.rows[0], b".a.");
    assert_eq!(second.board.rows[1], b".s.");
    assert_eq!(second.piece.cells.len(), 2);

    assert!(parser.next_turn().unwrap().is_none());
}

#[test]
fn parses_player_two_symbols() {
    let input = b"$$$ exec p2 : [/filler/solution/filler]\n";
    let mut parser = Parser::new(Cursor::new(input));
    let player = parser.read_player().unwrap();
    assert_eq!(player.number, 2);
    assert!(player.is_own(b'$'));
    assert!(player.is_own(b's'));
    assert!(player.is_enemy(b'@'));
    assert!(player.is_enemy(b'a'));
}

#[test]
fn lowercase_last_piece_markers_are_used_in_placement() {
    let board = board(&["a.s"]);
    let piece = piece(&["O"]);
    let player_one = Player::new(1).unwrap();
    let player_two = Player::new(2).unwrap();

    assert!(validate_placement(
        &board,
        &piece,
        player_one,
        Position { x: 0, y: 0 }
    ));
    assert!(!validate_placement(
        &board,
        &piece,
        player_one,
        Position { x: 2, y: 0 }
    ));
    assert!(validate_placement(
        &board,
        &piece,
        player_two,
        Position { x: 2, y: 0 }
    ));
    assert!(!validate_placement(
        &board,
        &piece,
        player_two,
        Position { x: 0, y: 0 }
    ));
}

#[test]
fn accepts_exactly_one_own_overlap() {
    let board = board(&[".....", "..@..", ".....", "...$.", "....."]);
    let piece = piece(&["OO", ".O"]);
    let player = Player::new(1).unwrap();
    assert!(validate_placement(
        &board,
        &piece,
        player,
        Position { x: 2, y: 1 }
    ));
}

#[test]
fn rejects_zero_own_overlaps() {
    let board = board(&[".....", "..@..", ".....", "...$.", "....."]);
    let piece = piece(&["OO"]);
    let player = Player::new(1).unwrap();
    assert!(!validate_placement(
        &board,
        &piece,
        player,
        Position { x: 0, y: 0 }
    ));
}

#[test]
fn rejects_two_own_overlaps() {
    let board = board(&["@@...", "....."]);
    let piece = piece(&["OO"]);
    let player = Player::new(1).unwrap();
    assert!(!validate_placement(
        &board,
        &piece,
        player,
        Position { x: 0, y: 0 }
    ));
}

#[test]
fn rejects_opponent_overlap() {
    let board = board(&["@$...", "....."]);
    let piece = piece(&["OO"]);
    let player = Player::new(1).unwrap();
    assert!(!validate_placement(
        &board,
        &piece,
        player,
        Position { x: 0, y: 0 }
    ));
}

#[test]
fn rejects_occupied_piece_cells_past_each_board_edge() {
    let player = Player::new(1).unwrap();
    let horizontal = piece(&["OO"]);
    let vertical = piece(&["O", "O"]);

    let left = board(&[".....", "@....", "....."]);
    assert!(!validate_placement(
        &left,
        &horizontal,
        player,
        Position { x: -1, y: 1 }
    ));

    let right = board(&[".....", "....@", "....."]);
    assert!(!validate_placement(
        &right,
        &horizontal,
        player,
        Position { x: 4, y: 1 }
    ));

    let top = board(&["..@..", ".....", "....."]);
    assert!(!validate_placement(
        &top,
        &vertical,
        player,
        Position { x: 2, y: -1 }
    ));

    let bottom = board(&[".....", ".....", "..@.."]);
    assert!(!validate_placement(
        &bottom,
        &vertical,
        player,
        Position { x: 2, y: 2 }
    ));
}

#[test]
fn ignores_empty_padding_when_validating_boundaries() {
    let board = board(&["@....", "....."]);
    let piece = piece(&[".O"]);
    let player = Player::new(1).unwrap();
    assert!(validate_placement(
        &board,
        &piece,
        player,
        Position { x: -1, y: 0 }
    ));
}

#[test]
fn generated_placements_are_all_legal() {
    let board = board(&[".......", "..@....", ".......", ".....$.", "......."]);
    let piece = piece(&[".OO", "OO."]);
    let player = Player::new(1).unwrap();
    let placements = generate_placements(&board, &piece, player);
    assert!(!placements.is_empty());
    assert!(placements
        .iter()
        .all(|position| validate_placement(&board, &piece, player, *position)));
}

#[test]
fn generated_placements_include_every_legal_origin() {
    let board = board(&[".....", ".@...", ".....", "...$.", "....."]);
    let piece = piece(&[".OO", "O.."]);
    let player = Player::new(1).unwrap();

    let generated: BTreeSet<Position> = generate_placements(&board, &piece, player)
        .into_iter()
        .collect();
    let mut expected = BTreeSet::new();

    for y in -(piece.height as i32)..=board.height as i32 {
        for x in -(piece.width as i32)..=board.width as i32 {
            let position = Position { x, y };
            if validate_placement(&board, &piece, player, position) {
                expected.insert(position);
            }
        }
    }

    assert!(!expected.is_empty());
    assert_eq!(generated, expected);
}

#[test]
fn formats_coordinates_exactly_for_game_engine() {
    assert_eq!(format_move(Position { x: -2, y: 17 }), "-2 17\n");
}

#[test]
fn strategy_returns_a_legal_move() {
    let board = board(&[
        "........", "..@.....", "........", "........", ".....$..", "........",
    ]);
    let piece = piece(&[".OO", ".O."]);
    let player = Player::new(1).unwrap();
    let chosen = Strategy::new().choose(&board, &piece, player).unwrap();
    assert!(validate_placement(&board, &piece, player, chosen));
}

#[test]
fn strategy_returns_none_when_no_move_exists() {
    let board = board(&["@$"]);
    let piece = piece(&["OO"]);
    let player = Player::new(1).unwrap();
    assert_eq!(Strategy::new().choose(&board, &piece, player), None);
}

#[test]
fn no_move_falls_back_to_zero_zero_output() {
    let board = board(&["@$"]);
    let piece = piece(&["OO"]);
    let player = Player::new(1).unwrap();
    let position = Strategy::new()
        .choose(&board, &piece, player)
        .unwrap_or(Position { x: 0, y: 0 });

    assert_eq!(format_move(position), "0 0\n");
}

#[test]
fn player_two_can_generate_legal_moves() {
    let board = board(&[".......", "..@....", ".......", "....$..", "......."]);
    let piece = piece(&["OO"]);
    let player = Player::new(2).unwrap();
    let placements = generate_placements(&board, &piece, player);
    assert!(!placements.is_empty());
    assert!(placements
        .iter()
        .all(|position| validate_placement(&board, &piece, player, *position)));
}
