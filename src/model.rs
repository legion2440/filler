use std::io;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Board {
    pub width: usize,
    pub height: usize,
    pub rows: Vec<Vec<u8>>,
}

impl Board {
    pub fn at(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return 0;
        }
        self.rows[y as usize][x as usize]
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Piece {
    pub width: usize,
    pub height: usize,
    pub rows: Vec<Vec<u8>>,
    pub cells: Vec<Position>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Turn {
    pub board: Board,
    pub piece: Piece,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Player {
    pub number: u8,
    own_stable: u8,
    own_last: u8,
    enemy_stable: u8,
    enemy_last: u8,
}

impl Player {
    pub fn new(number: u8) -> io::Result<Self> {
        match number {
            1 => Ok(Self {
                number: 1,
                own_stable: b'@',
                own_last: b'a',
                enemy_stable: b'$',
                enemy_last: b's',
            }),
            2 => Ok(Self {
                number: 2,
                own_stable: b'$',
                own_last: b's',
                enemy_stable: b'@',
                enemy_last: b'a',
            }),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unsupported player number: {number}"),
            )),
        }
    }

    pub fn is_own(self, cell: u8) -> bool {
        cell == self.own_stable || cell == self.own_last
    }

    pub fn is_enemy(self, cell: u8) -> bool {
        cell == self.enemy_stable || cell == self.enemy_last
    }
}
