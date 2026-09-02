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
fn rejects_occupied_piece_cells_outside_board() {
    let board = board(&["@....", "....."]);
    let piece = piece(&["OO"]);
    let player = Player::new(1).unwrap();
    assert!(!validate_placement(
        &board,
        &piece,
        player,
        Position { x: -1, y: 0 }
    ));
}

#[test]
fn allows_empty_piece_padding_outside_board() {
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
fn formats_coordinates_exactly_for_game_engine() {
    assert_eq!(format_move(Position { x: -2, y: 17 }), "-2 17\n");
}

#[test]
fn strategy_returns_a_legal_move() {
    let board = board(&[
        "........",
        "..@.....",
        "........",
        "........",
        ".....$..",
        "........",
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
