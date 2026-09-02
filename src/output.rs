use crate::Position;

pub fn format_move(position: Position) -> String {
    format!("{} {}\n", position.x, position.y)
}
