use std::io::{self, BufReader, BufWriter, Write};

use filler::{format_move, Parser, Position, Strategy};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
    }
}

fn run() -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut parser = Parser::new(BufReader::new(stdin.lock()));
    let player = parser.read_player()?;
    let strategy = Strategy::new();
    let mut output = BufWriter::new(stdout.lock());

    loop {
        let turn = match parser.next_turn()? {
            Some(turn) => turn,
            None => return Ok(()),
        };
        let position = strategy
            .choose(&turn.board, &turn.piece, player)
            .unwrap_or(Position { x: 0, y: 0 });
        output.write_all(format_move(position).as_bytes())?;
        output.flush()?;
    }
}
