pub mod board;
pub mod piece;
pub mod srs;

use macroquad::miniquad::date;
use macroquad::prelude::*;

use crate::core::scene::{GameScene, SceneAction};
use crate::core::{VIRTUAL_HEIGHT, VIRTUAL_WIDTH};
use crate::games::tetris::board::{
    Board, GravityTimer, BOARD_WIDTH, BUFFER_ROWS, VISIBLE_HEIGHT,
};
use crate::games::tetris::piece::{ActivePiece, PieceKind, SevenBag};
use crate::games::tetris::srs::{try_rotate, RotationDir};

const CELL_SIZE: f32 = 32.0;
const BOARD_PIXEL_W: f32 = BOARD_WIDTH as f32 * CELL_SIZE;
const BOARD_PIXEL_H: f32 = VISIBLE_HEIGHT as f32 * CELL_SIZE;
const BOARD_X: f32 = (VIRTUAL_WIDTH - BOARD_PIXEL_W) * 0.5;
const BOARD_Y: f32 = (VIRTUAL_HEIGHT - BOARD_PIXEL_H) * 0.5;

const DAS_DELAY: f32 = 0.15;
const ARR_INTERVAL: f32 = 0.045;
const LOCK_DELAY: f32 = 0.50;
const MAX_LOCK_RESETS: u8 = 15;

const BG_COLOR: Color = Color::new(0.045, 0.055, 0.10, 1.0);
const BOARD_BG: Color = Color::new(0.03, 0.04, 0.075, 1.0);
const PANEL_BG: Color = Color::new(0.075, 0.095, 0.16, 1.0);
const GRID_LINE: Color = Color::new(0.11, 0.14, 0.22, 1.0);
const BORDER_COLOR: Color = Color::new(0.24, 0.30, 0.46, 1.0);
const ACCENT_CYAN: Color = Color::new(0.0, 0.88, 0.96, 1.0);
const ACCENT_GOLD: Color = Color::new(1.0, 0.82, 0.22, 1.0);
const TEXT_PRIMARY: Color = Color::new(0.95, 0.96, 0.99, 1.0);
const TEXT_MUTED: Color = Color::new(0.58, 0.65, 0.78, 1.0);

/// Stack-allocated decimal formatter to avoid heap allocations in `draw()`.
struct NumBuf {
    buf: [u8; 16],
    start: usize,
}

impl NumBuf {
    fn from_u32(mut value: u32) -> Self {
        let mut buf = [b'0'; 16];
        let mut i = 16;
        if value == 0 {
            i = 15;
        } else {
            while value > 0 && i > 0 {
                i -= 1;
                buf[i] = b'0' + (value % 10) as u8;
                value /= 10;
            }
        }
        Self { buf, start: i }
    }

    fn as_str(&self) -> &str {
        std::str::from_utf8(&self.buf[self.start..]).unwrap_or("0")
    }
}

/// Pure Rust Tetris scene implementing SRS, 7-bag RNG, Ghost piece, Hold slot, and HUD.
pub struct TetrisScene {
    board: Board,
    bag: SevenBag,
    active: ActivePiece,
    hold: Option<PieceKind>,
    can_hold: bool,
    gravity: GravityTimer,
    score: u32,
    lines: u32,
    level: u32,
    lock_timer: f32,
    lock_resets: u8,
    das_dir: i32,
    das_timer: f32,
    arr_timer: f32,
    last_clear_label: &'static str,
    clear_banner_timer: f32,
    paused: bool,
    game_over: bool,
}

impl TetrisScene {
    pub fn new() -> Self {
        let seed = date::now().to_bits() ^ 0xA5A5_5A5A_1234_5678;
        let mut bag = SevenBag::new(seed);
        let first = bag.next_piece();
        let board = Board::new();
        let active = Self::spawn_for_board(&board, first).unwrap_or(ActivePiece::spawn(first, 0));

        Self {
            board,
            bag,
            active,
            hold: None,
            can_hold: true,
            gravity: GravityTimer::new(),
            score: 0,
            lines: 0,
            level: 1,
            lock_timer: 0.0,
            lock_resets: 0,
            das_dir: 0,
            das_timer: 0.0,
            arr_timer: 0.0,
            last_clear_label: "",
            clear_banner_timer: 0.0,
            paused: false,
            game_over: false,
        }
    }

    fn reset(&mut self) {
        let seed = date::now().to_bits() ^ ((self.score as u64).wrapping_mul(0x9E37_79B9));
        self.board.clear();
        self.bag = SevenBag::new(seed);
        self.hold = None;
        self.can_hold = true;
        self.gravity.reset();
        self.score = 0;
        self.lines = 0;
        self.level = 1;
        self.lock_timer = 0.0;
        self.lock_resets = 0;
        self.das_dir = 0;
        self.das_timer = 0.0;
        self.arr_timer = 0.0;
        self.last_clear_label = "";
        self.clear_banner_timer = 0.0;
        self.paused = false;
        self.game_over = false;

        let first = self.bag.next_piece();
        self.active =
            Self::spawn_for_board(&self.board, first).unwrap_or(ActivePiece::spawn(first, 0));
    }

    fn spawn_for_board(board: &Board, kind: PieceKind) -> Option<ActivePiece> {
        let preferred = ActivePiece::spawn(kind, 1);
        if board.fits(&preferred) {
            return Some(preferred);
        }
        let fallback = ActivePiece::spawn(kind, 0);
        if board.fits(&fallback) {
            return Some(fallback);
        }
        None
    }

    fn spawn_next_piece(&mut self) {
        let kind = self.bag.next_piece();
        self.gravity.reset();
        self.lock_timer = 0.0;
        self.lock_resets = 0;
        if let Some(piece) = Self::spawn_for_board(&self.board, kind) {
            self.active = piece;
        } else {
            self.active = ActivePiece::spawn(kind, 0);
            self.game_over = true;
        }
    }

    fn is_grounded(&self) -> bool {
        !self.board.fits(&ActivePiece {
            y: self.active.y + 1,
            ..self.active
        })
    }

    fn reset_lock_if_allowed(&mut self) {
        if self.lock_timer > 0.0 && self.lock_resets < MAX_LOCK_RESETS {
            self.lock_timer = 0.0;
            self.lock_resets += 1;
        }
    }

    fn try_shift(&mut self, dx: i32) -> bool {
        let candidate = ActivePiece {
            x: self.active.x + dx,
            ..self.active
        };
        if self.board.fits(&candidate) {
            self.active = candidate;
            self.reset_lock_if_allowed();
            true
        } else {
            false
        }
    }

    fn try_step_down(&mut self) -> bool {
        let candidate = ActivePiece {
            y: self.active.y + 1,
            ..self.active
        };
        if self.board.fits(&candidate) {
            self.active = candidate;
            self.lock_timer = 0.0;
            true
        } else {
            false
        }
    }

    fn handle_rotation(&mut self, dir: RotationDir) {
        if let Some(rotated) = try_rotate(&self.board, self.active, dir) {
            self.active = rotated;
            self.reset_lock_if_allowed();
        }
    }

    fn handle_hold(&mut self) {
        if !self.can_hold {
            return;
        }
        self.can_hold = false;
        let current_kind = self.active.kind;
        let next_kind = match self.hold {
            Some(held) => {
                self.hold = Some(current_kind);
                held
            }
            None => {
                self.hold = Some(current_kind);
                self.bag.next_piece()
            }
        };

        self.gravity.reset();
        self.lock_timer = 0.0;
        self.lock_resets = 0;
        if let Some(piece) = Self::spawn_for_board(&self.board, next_kind) {
            self.active = piece;
        } else {
            self.active = ActivePiece::spawn(next_kind, 0);
            self.game_over = true;
        }
    }

    fn lock_and_advance(&mut self) {
        self.board.lock_piece(&self.active);
        let cleared = self.board.clear_lines();
        if cleared > 0 {
            let points = match cleared {
                1 => {
                    self.last_clear_label = "SINGLE";
                    100
                }
                2 => {
                    self.last_clear_label = "DOUBLE";
                    300
                }
                3 => {
                    self.last_clear_label = "TRIPLE";
                    500
                }
                _ => {
                    self.last_clear_label = "TETRIS!";
                    800
                }
            };
            self.score = self.score.saturating_add(points * self.level);
            self.lines = self.lines.saturating_add(cleared);
            self.level = (self.lines / 10) + 1;
            self.clear_banner_timer = 1.4;
        }
        self.can_hold = true;
        self.spawn_next_piece();
    }

    fn handle_hard_drop(&mut self) {
        let target_y = self.board.ghost_y(&self.active);
        let dropped = (target_y - self.active.y).max(0) as u32;
        self.active.y = target_y;
        self.score = self.score.saturating_add(dropped * 2);
        self.lock_and_advance();
    }

    fn handle_horizontal_input(&mut self, dt: f32) {
        let left_pressed = is_key_pressed(KeyCode::Left);
        let right_pressed = is_key_pressed(KeyCode::Right);
        let left_down = is_key_down(KeyCode::Left);
        let right_down = is_key_down(KeyCode::Right);

        if left_pressed {
            self.das_dir = -1;
            self.das_timer = 0.0;
            self.arr_timer = 0.0;
            self.try_shift(-1);
            return;
        }
        if right_pressed {
            self.das_dir = 1;
            self.das_timer = 0.0;
            self.arr_timer = 0.0;
            self.try_shift(1);
            return;
        }

        let desired_dir = if self.das_dir == -1 && left_down {
            -1
        } else if self.das_dir == 1 && right_down {
            1
        } else if left_down && !right_down {
            -1
        } else if right_down && !left_down {
            1
        } else {
            0
        };

        if desired_dir == 0 {
            self.das_dir = 0;
            self.das_timer = 0.0;
            self.arr_timer = 0.0;
            return;
        }

        if desired_dir != self.das_dir {
            self.das_dir = desired_dir;
            self.das_timer = 0.0;
            self.arr_timer = 0.0;
            self.try_shift(desired_dir);
            return;
        }

        self.das_timer += dt;
        if self.das_timer >= DAS_DELAY {
            self.arr_timer += dt;
            while self.arr_timer >= ARR_INTERVAL {
                self.arr_timer -= ARR_INTERVAL;
                if !self.try_shift(desired_dir) {
                    self.arr_timer = 0.0;
                    break;
                }
            }
        }
    }

    fn draw_block(x: f32, y: f32, size: f32, color: Color) {
        draw_rectangle(x + 1.0, y + 1.0, size - 2.0, size - 2.0, color);
        // Subtle top-left highlight & bottom-right bevel for crisp retro blocks
        let highlight = Color::new(
            (color.r + 0.28).min(1.0),
            (color.g + 0.28).min(1.0),
            (color.b + 0.28).min(1.0),
            color.a,
        );
        let shadow = Color::new(color.r * 0.55, color.g * 0.55, color.b * 0.55, color.a);
        draw_rectangle(x + 1.0, y + 1.0, size - 2.0, 3.0, highlight);
        draw_rectangle(x + 1.0, y + 1.0, 3.0, size - 2.0, highlight);
        draw_rectangle(x + 1.0, y + size - 4.0, size - 2.0, 3.0, shadow);
        draw_rectangle(x + size - 4.0, y + 1.0, 3.0, size - 2.0, shadow);
    }

    fn draw_mini_piece(kind: PieceKind, center_x: f32, center_y: f32, cell: f32, alpha: f32) {
        let cells = kind.cells(0);
        let mut min_x = 4;
        let mut max_x = 0;
        let mut min_y = 4;
        let mut max_y = 0;
        for (cx, cy) in cells {
            if cx < min_x {
                min_x = cx;
            }
            if cx > max_x {
                max_x = cx;
            }
            if cy < min_y {
                min_y = cy;
            }
            if cy > max_y {
                max_y = cy;
            }
        }

        let piece_w = (max_x - min_x + 1) as f32 * cell;
        let piece_h = (max_y - min_y + 1) as f32 * cell;
        let origin_x = center_x - piece_w * 0.5;
        let origin_y = center_y - piece_h * 0.5;

        let base = kind.color();
        let color = Color::new(base.r, base.g, base.b, alpha);
        for (cx, cy) in cells {
            let px = origin_x + (cx - min_x) as f32 * cell;
            let py = origin_y + (cy - min_y) as f32 * cell;
            Self::draw_block(px, py, cell, color);
        }
    }

    fn draw_centered_in_board(text: &str, y: f32, font_size: f32, color: Color) {
        let dims = measure_text(text, None, font_size as u16, 1.0);
        let x = BOARD_X + (BOARD_PIXEL_W - dims.width) * 0.5;
        draw_text(text, x, y, font_size, color);
    }

    fn draw_panel(x: f32, y: f32, w: f32, h: f32, title: &str) {
        draw_rectangle(x, y, w, h, PANEL_BG);
        draw_rectangle_lines(x, y, w, h, 2.0, BORDER_COLOR);
        draw_text(title, x + 14.0, y + 26.0, 20.0, ACCENT_CYAN);
        draw_line(x + 10.0, y + 34.0, x + w - 10.0, y + 34.0, 1.0, BORDER_COLOR);
    }
}

impl Default for TetrisScene {
    fn default() -> Self {
        Self::new()
    }
}

impl GameScene for TetrisScene {
    fn update(&mut self) -> SceneAction {
        let dt = get_frame_time().min(0.1);

        if self.clear_banner_timer > 0.0 {
            self.clear_banner_timer = (self.clear_banner_timer - dt).max(0.0);
        }

        if self.game_over {
            if is_key_pressed(KeyCode::R) || is_key_pressed(KeyCode::Enter) {
                self.reset();
            }
            return SceneAction::None;
        }

        if is_key_pressed(KeyCode::P) {
            self.paused = !self.paused;
        }
        if self.paused {
            return SceneAction::None;
        }

        // Hold piece (C or Left/Right Shift)
        if is_key_pressed(KeyCode::C)
            || is_key_pressed(KeyCode::LeftShift)
            || is_key_pressed(KeyCode::RightShift)
        {
            self.handle_hold();
            if self.game_over {
                return SceneAction::None;
            }
        }

        // Rotations: Up / X = CW, Z / LeftControl / RightControl = CCW
        if is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::X) {
            self.handle_rotation(RotationDir::Cw);
        } else if is_key_pressed(KeyCode::Z)
            || is_key_pressed(KeyCode::LeftControl)
            || is_key_pressed(KeyCode::RightControl)
        {
            self.handle_rotation(RotationDir::Ccw);
        }

        // Horizontal movement with DAS/ARR
        self.handle_horizontal_input(dt);

        // Hard drop (Space)
        if is_key_pressed(KeyCode::Space) {
            self.handle_hard_drop();
            return SceneAction::None;
        }

        // Gravity & Soft drop (Down)
        let soft_drop = is_key_down(KeyCode::Down);
        let steps = self.gravity.advance(dt, self.level, soft_drop);
        for _ in 0..steps {
            if self.try_step_down() {
                if soft_drop {
                    self.score = self.score.saturating_add(1);
                }
            } else {
                break;
            }
        }

        // Lock delay handling when piece rests on stack or floor
        if self.is_grounded() {
            self.lock_timer += dt;
            if self.lock_timer >= LOCK_DELAY {
                self.lock_and_advance();
            }
        } else {
            self.lock_timer = 0.0;
        }

        SceneAction::None
    }

    fn draw(&self) {
        clear_background(BG_COLOR);

        // --- LEFT COLUMN: HOLD + STATS + CONTROLS ---
        let left_x = 84.0;
        let side_w = 200.0;

        // Hold slot panel
        Self::draw_panel(left_x, 40.0, side_w, 136.0, "HOLD");
        if let Some(held) = self.hold {
            let alpha = if self.can_hold { 1.0 } else { 0.35 };
            Self::draw_mini_piece(held, left_x + side_w * 0.5, 124.0, 22.0, alpha);
        }

        // Stats panel (Score, Level, Lines)
        Self::draw_panel(left_x, 198.0, side_w, 224.0, "STATS");
        let score_buf = NumBuf::from_u32(self.score);
        let level_buf = NumBuf::from_u32(self.level);
        let lines_buf = NumBuf::from_u32(self.lines);

        draw_text("SCORE", left_x + 16.0, 258.0, 18.0, TEXT_MUTED);
        draw_text(score_buf.as_str(), left_x + 16.0, 286.0, 28.0, ACCENT_GOLD);

        draw_text("LEVEL", left_x + 16.0, 324.0, 18.0, TEXT_MUTED);
        draw_text(level_buf.as_str(), left_x + 16.0, 352.0, 28.0, TEXT_PRIMARY);

        draw_text("LINES", left_x + 16.0, 390.0, 18.0, TEXT_MUTED);
        draw_text(lines_buf.as_str(), left_x + 16.0, 414.0, 26.0, TEXT_PRIMARY);

        // Controls panel
        Self::draw_panel(left_x, 444.0, side_w, 236.0, "CONTROLS");
        const CONTROLS: [&str; 8] = [
            "LEFT/RIGHT : MOVE",
            "UP / X     : ROT CW",
            "Z / CTRL   : ROT CCW",
            "DOWN       : SOFT DROP",
            "SPACE      : HARD DROP",
            "C / SHIFT  : HOLD",
            "P          : PAUSE",
            "ESC        : MENU",
        ];
        for (i, line) in CONTROLS.iter().enumerate() {
            draw_text(line, left_x + 14.0, 498.0 + i as f32 * 22.0, 16.0, TEXT_MUTED);
        }

        // --- CENTER: 10x20 PLAYFIELD ---
        draw_rectangle(BOARD_X, BOARD_Y, BOARD_PIXEL_W, BOARD_PIXEL_H, BOARD_BG);

        // Subtle playfield grid lines
        for col in 1..BOARD_WIDTH {
            let x = BOARD_X + col as f32 * CELL_SIZE;
            draw_line(x, BOARD_Y, x, BOARD_Y + BOARD_PIXEL_H, 1.0, GRID_LINE);
        }
        for row in 1..VISIBLE_HEIGHT {
            let y = BOARD_Y + row as f32 * CELL_SIZE;
            draw_line(BOARD_X, y, BOARD_X + BOARD_PIXEL_W, y, 1.0, GRID_LINE);
        }

        // Locked board cells (rows BUFFER_ROWS..TOTAL_HEIGHT are visible)
        for vis_row in 0..VISIBLE_HEIGHT {
            let board_row = vis_row + BUFFER_ROWS;
            for col in 0..BOARD_WIDTH {
                if let Some(kind) = self.board.cell(col, board_row) {
                    let px = BOARD_X + col as f32 * CELL_SIZE;
                    let py = BOARD_Y + vis_row as f32 * CELL_SIZE;
                    Self::draw_block(px, py, CELL_SIZE, kind.color());
                }
            }
        }

        // Ghost piece & active piece (only while not game over)
        if !self.game_over {
            let ghost_y = self.board.ghost_y(&self.active);
            let ghost_piece = ActivePiece {
                y: ghost_y,
                ..self.active
            };
            let piece_color = self.active.kind.color();
            let ghost_fill = Color::new(piece_color.r, piece_color.g, piece_color.b, 0.16);
            let ghost_border = Color::new(piece_color.r, piece_color.g, piece_color.b, 0.55);

            for (cx, cy) in ghost_piece.world_cells() {
                if cy >= BUFFER_ROWS as i32 {
                    let vis_y = (cy - BUFFER_ROWS as i32) as f32;
                    let px = BOARD_X + cx as f32 * CELL_SIZE;
                    let py = BOARD_Y + vis_y * CELL_SIZE;
                    draw_rectangle(px + 2.0, py + 2.0, CELL_SIZE - 4.0, CELL_SIZE - 4.0, ghost_fill);
                    draw_rectangle_lines(
                        px + 2.0,
                        py + 2.0,
                        CELL_SIZE - 4.0,
                        CELL_SIZE - 4.0,
                        2.0,
                        ghost_border,
                    );
                }
            }

            for (cx, cy) in self.active.world_cells() {
                if cy >= BUFFER_ROWS as i32 {
                    let vis_y = (cy - BUFFER_ROWS as i32) as f32;
                    let px = BOARD_X + cx as f32 * CELL_SIZE;
                    let py = BOARD_Y + vis_y * CELL_SIZE;
                    Self::draw_block(px, py, CELL_SIZE, piece_color);
                }
            }
        }

        // Playfield outer border
        draw_rectangle_lines(
            BOARD_X - 2.0,
            BOARD_Y - 2.0,
            BOARD_PIXEL_W + 4.0,
            BOARD_PIXEL_H + 4.0,
            3.0,
            ACCENT_CYAN,
        );

        // --- RIGHT COLUMN: NEXT QUEUE + CLEAR BANNER ---
        let right_x = BOARD_X + BOARD_PIXEL_W + 36.0;
        Self::draw_panel(right_x, 40.0, side_w, 400.0, "NEXT");
        for i in 0..4 {
            let next_kind = self.bag.peek(i);
            let slot_cy = 114.0 + i as f32 * 86.0;
            Self::draw_mini_piece(next_kind, right_x + side_w * 0.5, slot_cy, 21.0, 1.0);
        }

        // Line clear feedback banner
        if self.clear_banner_timer > 0.0 && !self.last_clear_label.is_empty() {
            let alpha = (self.clear_banner_timer / 1.4).min(1.0);
            let banner_y = 468.0;
            draw_rectangle(
                right_x,
                banner_y,
                side_w,
                68.0,
                Color::new(0.10, 0.14, 0.24, alpha * 0.9),
            );
            draw_rectangle_lines(
                right_x,
                banner_y,
                side_w,
                68.0,
                2.0,
                Color::new(ACCENT_GOLD.r, ACCENT_GOLD.g, ACCENT_GOLD.b, alpha),
            );
            let dims = measure_text(self.last_clear_label, None, 28, 1.0);
            draw_text(
                self.last_clear_label,
                right_x + (side_w - dims.width) * 0.5,
                banner_y + 43.0,
                28.0,
                Color::new(ACCENT_GOLD.r, ACCENT_GOLD.g, ACCENT_GOLD.b, alpha),
            );
        }

        // --- OVERLAYS: PAUSED / GAME OVER ---
        if self.paused {
            draw_rectangle(
                BOARD_X,
                BOARD_Y,
                BOARD_PIXEL_W,
                BOARD_PIXEL_H,
                Color::new(0.02, 0.03, 0.06, 0.78),
            );
            Self::draw_centered_in_board("PAUSED", BOARD_Y + 290.0, 44.0, ACCENT_CYAN);
            Self::draw_centered_in_board("PRESS [P] TO RESUME", BOARD_Y + 340.0, 20.0, TEXT_PRIMARY);
        } else if self.game_over {
            draw_rectangle(
                BOARD_X,
                BOARD_Y,
                BOARD_PIXEL_W,
                BOARD_PIXEL_H,
                Color::new(0.05, 0.02, 0.04, 0.84),
            );
            Self::draw_centered_in_board(
                "GAME OVER",
                BOARD_Y + 270.0,
                44.0,
                Color::new(0.98, 0.28, 0.30, 1.0),
            );
            Self::draw_centered_in_board("PRESS [R] OR [ENTER]", BOARD_Y + 325.0, 20.0, ACCENT_GOLD);
            Self::draw_centered_in_board("TO PLAY AGAIN", BOARD_Y + 352.0, 20.0, TEXT_PRIMARY);
            Self::draw_centered_in_board("[ESC] RETURN TO MENU", BOARD_Y + 402.0, 18.0, TEXT_MUTED);
        }
    }
}
