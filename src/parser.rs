use std::io::{self, BufRead};

use crate::{Board, Piece, Player, Position, Turn};

pub struct Parser<R: BufRead> {
    reader: R,
    line: String,
}

impl<R: BufRead> Parser<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            line: String::with_capacity(4096),
        }
    }

    pub fn read_player(&mut self) -> io::Result<Player> {
        while let Some(line) = self.read_line()? {
            let trimmed = line.trim();
            if !trimmed.starts_with("$$$") {
                continue;
            }
            let marker = trimmed
                .split_whitespace()
                .find(|part| part.starts_with('p') && part.len() == 2);
            if let Some(marker) = marker {
                let number = marker.as_bytes()[1];
                if number == b'1' || number == b'2' {
                    return Player::new(number - b'0');
                }
            }
        }
        Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "player header not found",
        ))
    }

    pub fn next_turn(&mut self) -> io::Result<Option<Turn>> {
        let (width, height) = match self.find_dimensions("Anfield")? {
            Some(value) => value,
            None => return Ok(None),
        };
        let board_rows = self.read_board_rows(width, height)?;

        let (piece_width, piece_height) = self.find_dimensions("Piece")?.ok_or_else(|| {
            io::Error::new(io::ErrorKind::UnexpectedEof, "piece header not found")
        })?;
        let (piece_rows, piece_cells) = self.read_piece_rows(piece_width, piece_height)?;

        Ok(Some(Turn {
            board: Board {
                width,
                height,
                rows: board_rows,
            },
            piece: Piece {
                width: piece_width,
                height: piece_height,
                rows: piece_rows,
                cells: piece_cells,
            },
        }))
    }

    fn read_line(&mut self) -> io::Result<Option<String>> {
        self.line.clear();
        let bytes = self.reader.read_line(&mut self.line)?;
        if bytes == 0 {
            return Ok(None);
        }
        while self.line.ends_with('\n') || self.line.ends_with('\r') {
            self.line.pop();
        }
        Ok(Some(self.line.clone()))
    }

    fn find_dimensions(&mut self, prefix: &str) -> io::Result<Option<(usize, usize)>> {
        while let Some(line) = self.read_line()? {
            let trimmed = line.trim();
            let rest = match trimmed.strip_prefix(prefix) {
                Some(rest) => rest,
                None => continue,
            };
            let mut parts = rest.split_whitespace();
            let width = parts
                .next()
                .ok_or_else(|| invalid("missing width"))?
                .trim_end_matches(':')
                .parse::<usize>()
                .map_err(|_| invalid("invalid width"))?;
            let height = parts
                .next()
                .ok_or_else(|| invalid("missing height"))?
                .trim_end_matches(':')
                .parse::<usize>()
                .map_err(|_| invalid("invalid height"))?;
            if width == 0 || height == 0 {
                return Err(invalid("dimensions must be positive"));
            }
            return Ok(Some((width, height)));
        }
        Ok(None)
    }

    fn read_board_rows(&mut self, width: usize, height: usize) -> io::Result<Vec<Vec<u8>>> {
        let mut rows = Vec::with_capacity(height);
        while rows.len() < height {
            let line = self
                .read_line()?
                .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "incomplete board"))?;
            let mut parts = line.split_whitespace();
            let index = match parts.next() {
                Some(index) => index,
                None => continue,
            };
            let row = match parts.next() {
                Some(row) => row,
                None => continue,
            };
            if !index.as_bytes().iter().all(|cell| cell.is_ascii_digit()) {
                continue;
            }
            if row.len() != width || !row.bytes().all(is_board_cell) {
                return Err(invalid(&format!(
                    "invalid board row width/content: expected {}, got {}",
                    width,
                    row.len()
                )));
            }
            rows.push(row.as_bytes().to_vec());
        }
        Ok(rows)
    }

    fn read_piece_rows(
        &mut self,
        width: usize,
        height: usize,
    ) -> io::Result<(Vec<Vec<u8>>, Vec<Position>)> {
        let mut rows = Vec::with_capacity(height);
        let mut cells = Vec::new();

        for y in 0..height {
            let row = self
                .read_line()?
                .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "incomplete piece"))?;
            let trimmed = row.trim();
            if trimmed.len() != width {
                return Err(invalid(&format!(
                    "piece row width {}, expected {}",
                    trimmed.len(),
                    width
                )));
            }
            let bytes = trimmed.as_bytes().to_vec();
            for (x, cell) in bytes.iter().enumerate() {
                if *cell != b'.' {
                    cells.push(Position {
                        x: x as i32,
                        y: y as i32,
                    });
                }
            }
            rows.push(bytes);
        }

        if cells.is_empty() {
            return Err(invalid("piece has no occupied cells"));
        }
        Ok((rows, cells))
    }
}

fn is_board_cell(cell: u8) -> bool {
    matches!(cell, b'.' | b'@' | b'$' | b'a' | b's')
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_owned())
}
