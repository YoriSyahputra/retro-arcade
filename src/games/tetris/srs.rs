use crate::games::tetris::board::Board;
use crate::games::tetris::piece::{ActivePiece, PieceKind};

/// Rotation direction for Super Rotation System (SRS).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotationDir {
    Cw,
    Ccw,
}

impl RotationDir {
    #[inline]
    pub const fn apply(self, rotation: u8) -> u8 {
        match self {
            RotationDir::Cw => (rotation + 1) & 3,
            RotationDir::Ccw => (rotation + 3) & 3,
        }
    }
}

/// Official SRS wall-kick offset tables for J, L, S, T, Z pieces.
/// Coordinates use board space where +Y points downward (negated from guideline +Y up).
/// Indexed by transition:
/// 0: 0->1, 1: 1->0, 2: 1->2, 3: 2->1, 4: 2->3, 5: 3->2, 6: 3->0, 7: 0->3
const JLSTZ_KICKS: [[(i32, i32); 5]; 8] = [
    [(0, 0), (-1, 0), (-1, -1), (0, 2), (-1, 2)], // 0 -> 1
    [(0, 0), (1, 0), (1, 1), (0, -2), (1, -2)],   // 1 -> 0
    [(0, 0), (1, 0), (1, 1), (0, -2), (1, -2)],   // 1 -> 2
    [(0, 0), (-1, 0), (-1, -1), (0, 2), (-1, 2)], // 2 -> 1
    [(0, 0), (1, 0), (1, -1), (0, 2), (1, 2)],    // 2 -> 3
    [(0, 0), (-1, 0), (-1, 1), (0, -2), (-1, -2)],// 3 -> 2
    [(0, 0), (-1, 0), (-1, 1), (0, -2), (-1, -2)],// 3 -> 0
    [(0, 0), (1, 0), (1, -1), (0, 2), (1, 2)],    // 0 -> 3
];

/// Official SRS wall-kick offset tables for the I-piece (+Y downward).
const I_KICKS: [[(i32, i32); 5]; 8] = [
    [(0, 0), (-2, 0), (1, 0), (-2, 1), (1, -2)],  // 0 -> 1
    [(0, 0), (2, 0), (-1, 0), (2, -1), (-1, 2)],  // 1 -> 0
    [(0, 0), (-1, 0), (2, 0), (-1, -2), (2, 1)],  // 1 -> 2
    [(0, 0), (1, 0), (-2, 0), (1, 2), (-2, -1)],  // 2 -> 1
    [(0, 0), (2, 0), (-1, 0), (2, -1), (-1, 2)],  // 2 -> 3
    [(0, 0), (-2, 0), (1, 0), (-2, 1), (1, -2)],  // 3 -> 2
    [(0, 0), (1, 0), (-2, 0), (1, 2), (-2, -1)],  // 3 -> 0
    [(0, 0), (-1, 0), (2, 0), (-1, -2), (2, 1)],  // 0 -> 3
];

const O_KICKS: [(i32, i32); 5] = [(0, 0); 5];

/// Maps a `(from_rot, to_rot)` pair into the 8-entry SRS transition table index.
const fn transition_index(from: u8, to: u8) -> Option<usize> {
    match (from & 3, to & 3) {
        (0, 1) => Some(0),
        (1, 0) => Some(1),
        (1, 2) => Some(2),
        (2, 1) => Some(3),
        (2, 3) => Some(4),
        (3, 2) => Some(5),
        (3, 0) => Some(6),
        (0, 3) => Some(7),
        _ => None,
    }
}

/// Returns the 5 SRS translation offsets to test for a given piece and rotation transition.
pub const fn wall_kicks(kind: PieceKind, from_rot: u8, to_rot: u8) -> [(i32, i32); 5] {
    if matches!(kind, PieceKind::O) {
        return O_KICKS;
    }
    let Some(idx) = transition_index(from_rot, to_rot) else {
        return O_KICKS;
    };
    match kind {
        PieceKind::I => I_KICKS[idx],
        PieceKind::O => O_KICKS,
        _ => JLSTZ_KICKS[idx],
    }
}

/// Attempts to rotate `piece` on `board` using official SRS wall-kick tests.
/// Returns `Some(rotated_piece)` on the first valid kick test, or `None` if blocked.
pub fn try_rotate(board: &Board, piece: ActivePiece, dir: RotationDir) -> Option<ActivePiece> {
    let target_rot = dir.apply(piece.rotation);
    let kicks = wall_kicks(piece.kind, piece.rotation, target_rot);

    for (dx, dy) in kicks {
        let candidate = ActivePiece {
            kind: piece.kind,
            x: piece.x + dx,
            y: piece.y + dy,
            rotation: target_rot,
        };
        if board.fits(&candidate) {
            return Some(candidate);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::tetris::board::{BOARD_WIDTH, Board};

    #[test]
    fn wall_kick_moves_t_piece_off_left_wall() {
        let board = Board::new();
        // T-piece in state 1 (vertical bar on col 1, nub on col 2) flush against left wall (x = -1).
        let piece = ActivePiece {
            kind: PieceKind::T,
            x: -1,
            y: 10,
            rotation: 1,
        };
        assert!(board.fits(&piece));

        // Rotating CW to state 2 requires kicking right by +1 so local x=0 lands on col 0.
        let rotated = try_rotate(&board, piece, RotationDir::Cw).expect("SRS kick should succeed");
        assert_eq!(rotated.rotation, 2);
        assert_eq!(rotated.x, 0);
        assert!(board.fits(&rotated));
    }

    #[test]
    fn wall_kick_moves_i_piece_off_right_wall() {
        let board = Board::new();
        // Vertical I-piece in state 1 at rightmost column (local x=2 at col 9 => x = 7).
        let piece = ActivePiece {
            kind: PieceKind::I,
            x: BOARD_WIDTH as i32 - 3,
            y: 10,
            rotation: 1,
        };
        assert!(board.fits(&piece));

        let rotated = try_rotate(&board, piece, RotationDir::Cw).expect("I-piece SRS kick should succeed");
        assert_eq!(rotated.rotation, 2);
        assert!(board.fits(&rotated));
    }
}
