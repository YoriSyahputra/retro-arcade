use crate::games::tetris::piece::{ActivePiece, PieceKind};

/// Visible playfield width in columns.
pub const BOARD_WIDTH: usize = 10;
/// Visible playfield height in rows.
pub const VISIBLE_HEIGHT: usize = 20;
/// Hidden spawn buffer rows above the visible playfield.
pub const BUFFER_ROWS: usize = 2;
/// Total internal matrix height (visible + hidden buffer).
pub const TOTAL_HEIGHT: usize = VISIBLE_HEIGHT + BUFFER_ROWS;

/// Fixed-size 10x22 Tetris playfield matrix (zero heap allocations).
pub struct Board {
    cells: [[Option<PieceKind>; BOARD_WIDTH]; TOTAL_HEIGHT],
}

impl Board {
    pub const fn new() -> Self {
        Self {
            cells: [[None; BOARD_WIDTH]; TOTAL_HEIGHT],
        }
    }

    pub fn clear(&mut self) {
        self.cells = [[None; BOARD_WIDTH]; TOTAL_HEIGHT];
    }

    #[inline]
    pub const fn cell(&self, col: usize, row: usize) -> Option<PieceKind> {
        self.cells[row][col]
    }

    /// Returns true if all 4 cells of `piece` lie within board bounds and do not collide.
    pub fn fits(&self, piece: &ActivePiece) -> bool {
        for (x, y) in piece.world_cells() {
            if x < 0 || x >= BOARD_WIDTH as i32 || y < 0 || y >= TOTAL_HEIGHT as i32 {
                return false;
            }
            if self.cells[y as usize][x as usize].is_some() {
                return false;
            }
        }
        true
    }

    /// Writes the active piece's 4 cells permanently into the playfield matrix.
    pub fn lock_piece(&mut self, piece: &ActivePiece) {
        for (x, y) in piece.world_cells() {
            if x >= 0 && x < BOARD_WIDTH as i32 && y >= 0 && y < TOTAL_HEIGHT as i32 {
                self.cells[y as usize][x as usize] = Some(piece.kind);
            }
        }
    }

    /// Detects and removes all filled rows in-place, shifting rows above downward.
    /// Returns the number of cleared lines (0..=4).
    pub fn clear_lines(&mut self) -> u32 {
        let mut cleared = 0u32;
        let mut write_row = TOTAL_HEIGHT;

        for read_row in (0..TOTAL_HEIGHT).rev() {
            let full = self.cells[read_row].iter().all(Option::is_some);
            if full {
                cleared += 1;
            } else {
                write_row -= 1;
                if write_row != read_row {
                    self.cells[write_row] = self.cells[read_row];
                }
            }
        }

        for row in 0..write_row {
            self.cells[row] = [None; BOARD_WIDTH];
        }

        cleared
    }

    /// Computes the lowest valid `y` coordinate where `piece` would land on hard drop.
    pub fn ghost_y(&self, piece: &ActivePiece) -> i32 {
        let mut probe = *piece;
        while self.fits(&ActivePiece {
            y: probe.y + 1,
            ..probe
        }) {
            probe.y += 1;
        }
        probe.y
    }
}

impl Default for Board {
    fn default() -> Self {
        Self::new()
    }
}

/// Computes the gravity drop interval (seconds per row) for a 1-indexed level.
/// Follows the classic Guideline progressive speed curve (faster every 10 lines).
pub fn gravity_interval(level: u32) -> f32 {
    let lv = level.saturating_sub(1).min(20) as i32;
    let base = 0.8 - (lv as f32) * 0.007;
    base.powi(lv).max(0.02)
}

/// Manages frame-rate independent downward gravity accumulation.
pub struct GravityTimer {
    accumulator: f32,
}

impl GravityTimer {
    pub const fn new() -> Self {
        Self { accumulator: 0.0 }
    }

    pub fn reset(&mut self) {
        self.accumulator = 0.0;
    }

    /// Advances the timer by `dt` seconds and returns how many rows the piece should fall.
    pub fn advance(&mut self, dt: f32, level: u32, soft_drop: bool) -> u32 {
        let normal = gravity_interval(level);
        let interval = if soft_drop {
            (normal / 20.0).min(0.035)
        } else {
            normal
        };

        self.accumulator += dt;
        let mut steps = 0u32;
        while self.accumulator >= interval {
            self.accumulator -= interval;
            steps += 1;
            if steps >= VISIBLE_HEIGHT as u32 {
                self.accumulator = 0.0;
                break;
            }
        }
        steps
    }
}

impl Default for GravityTimer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clear_lines_compacts_matrix_correctly() {
        let mut board = Board::new();
        // Fill bottom 2 rows completely
        for row in (TOTAL_HEIGHT - 2)..TOTAL_HEIGHT {
            for col in 0..BOARD_WIDTH {
                board.cells[row][col] = Some(PieceKind::I);
            }
        }
        // Place one marker block right above them
        board.cells[TOTAL_HEIGHT - 3][4] = Some(PieceKind::T);

        let cleared = board.clear_lines();
        assert_eq!(cleared, 2);
        assert_eq!(board.cell(4, TOTAL_HEIGHT - 1), Some(PieceKind::T));
        assert_eq!(board.cell(4, TOTAL_HEIGHT - 3), None);
    }

    #[test]
    fn ghost_y_reaches_floor_on_empty_board() {
        let board = Board::new();
        let piece = ActivePiece::spawn(PieceKind::O, 0);
        // O-piece occupies local rows 0 and 1, so max y is TOTAL_HEIGHT - 2.
        assert_eq!(board.ghost_y(&piece), (TOTAL_HEIGHT - 2) as i32);
    }

    #[test]
    fn gravity_gets_faster_with_level() {
        assert!(gravity_interval(5) < gravity_interval(1));
        assert!(gravity_interval(15) < gravity_interval(5));
        assert!(gravity_interval(25) >= 0.02);
    }
}
