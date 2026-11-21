pub mod entity;
pub mod modes;

use macroquad::miniquad::date;
use macroquad::prelude::*;

use crate::core::scene::{GameScene, SceneAction};
use crate::core::{VIRTUAL_HEIGHT, VIRTUAL_WIDTH};
use crate::games::snake::entity::{
    Direction, EatOutcome, FruitField, Snake, SpecialKind, StepOutcome, CHILI_BOOST_DURATION,
    CHILI_PEPPER_POINTS, CHILI_SPEED_MULTIPLIER, GOLDEN_APPLE_GROWTH, GOLDEN_APPLE_POINTS,
    GRID_SIZE, RED_APPLE_GROWTH, RED_APPLE_POINTS,
};
use crate::games::snake::modes::{SnakeModifiers, WallMode};

const CELL_SIZE: f32 = 30.0;
const BOARD_PIXEL_SIZE: f32 = GRID_SIZE as f32 * CELL_SIZE; // 600.0
const BOARD_X: f32 = 56.0;
const BOARD_Y: f32 = (VIRTUAL_HEIGHT - BOARD_PIXEL_SIZE) * 0.5; // 60.0

const BASE_TICK_INTERVAL: f32 = 0.135;

const BG_COLOR: Color = Color::new(0.045, 0.055, 0.10, 1.0);
const PANEL_BG: Color = Color::new(0.075, 0.095, 0.16, 1.0);
const PANEL_SELECTED_BG: Color = Color::new(0.11, 0.16, 0.25, 1.0);
const GRID_LIGHT: Color = Color::new(0.065, 0.09, 0.14, 1.0);
const GRID_DARK: Color = Color::new(0.05, 0.072, 0.115, 1.0);
const BORDER_DIM: Color = Color::new(0.22, 0.28, 0.42, 1.0);
const ACCENT_GREEN: Color = Color::new(0.24, 0.90, 0.42, 1.0);
const ACCENT_PORTAL: Color = Color::new(0.68, 0.34, 0.98, 1.0);
const ACCENT_CLASSIC: Color = Color::new(0.95, 0.30, 0.30, 1.0);
const ACCENT_GOLD: Color = Color::new(1.0, 0.82, 0.22, 1.0);
const ACCENT_CHILI: Color = Color::new(1.0, 0.38, 0.14, 1.0);
const TEXT_PRIMARY: Color = Color::new(0.95, 0.96, 0.99, 1.0);
const TEXT_MUTED: Color = Color::new(0.58, 0.65, 0.78, 1.0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SnakePhase {
    ModifierSelect,
    Playing,
    Paused,
    GameOver,
    Victory,
}

/// Stack-allocated integer formatter (zero heap allocations).
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

    /// Formats a non-negative seconds value like `6.4s` into a fixed stack buffer.
    fn from_tenths_sec(seconds: f32) -> Self {
        let tenths = (seconds.max(0.0) * 10.0).ceil() as u32;
        let whole = tenths / 10;
        let frac = tenths % 10;

        let mut buf = [b'0'; 16];
        buf[15] = b's';
        buf[14] = b'0' + frac as u8;
        buf[13] = b'.';

        let mut i = 13;
        let mut w = whole;
        if w == 0 {
            i = 12;
            buf[12] = b'0';
        } else {
            while w > 0 && i > 0 {
                i -= 1;
                buf[i] = b'0' + (w % 10) as u8;
                w /= 10;
            }
        }
        Self { buf, start: i }
    }

    fn as_str(&self) -> &str {
        std::str::from_utf8(&self.buf[self.start..]).unwrap_or("0")
    }
}

/// Pure Rust Snake scene with Google-style modifiers, special fruits, and pre-game config UI.
pub struct SnakeScene {
    phase: SnakePhase,
    modifiers: SnakeModifiers,
    menu_cursor: usize,
    snake: Snake,
    fruits: FruitField,
    score: u32,
    high_score: u32,
    tick_accumulator: f32,
    chili_boost_timer: f32,
    elapsed: f32,
    status_flash: &'static str,
    status_flash_timer: f32,
}

impl SnakeScene {
    pub fn new() -> Self {
        let seed = date::now().to_bits() ^ 0x51A4_E9C3_7721_90AB;
        Self {
            phase: SnakePhase::ModifierSelect,
            modifiers: SnakeModifiers::new(),
            menu_cursor: 0,
            snake: Snake::new(),
            fruits: FruitField::new(seed),
            score: 0,
            high_score: 0,
            tick_accumulator: 0.0,
            chili_boost_timer: 0.0,
            elapsed: 0.0,
            status_flash: "",
            status_flash_timer: 0.0,
        }
    }

    fn start_run(&mut self) {
        let seed = date::now().to_bits() ^ ((self.score as u64).wrapping_mul(0x9E37_79B9) | 1);
        self.snake.reset();
        self.fruits.reset(&self.snake, self.modifiers, seed);
        self.score = 0;
        self.tick_accumulator = 0.0;
        self.chili_boost_timer = 0.0;
        self.status_flash = "";
        self.status_flash_timer = 0.0;
        self.phase = SnakePhase::Playing;
    }

    fn toggle_selected_modifier(&mut self) {
        match self.menu_cursor {
            0 => self.modifiers.wall_mode = self.modifiers.wall_mode.toggle(),
            1 => self.modifiers.fruit_multiplier = self.modifiers.fruit_multiplier.toggle(),
            2 => self.modifiers.special_items = !self.modifiers.special_items,
            _ => {}
        }
    }

    fn update_modifier_select(&mut self) {
        if is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::W) {
            self.menu_cursor = if self.menu_cursor == 0 {
                3
            } else {
                self.menu_cursor - 1
            };
        } else if is_key_pressed(KeyCode::Down) || is_key_pressed(KeyCode::S) {
            self.menu_cursor = (self.menu_cursor + 1) % 4;
        }

        if is_key_pressed(KeyCode::Left)
            || is_key_pressed(KeyCode::Right)
            || is_key_pressed(KeyCode::A)
            || is_key_pressed(KeyCode::D)
        {
            self.toggle_selected_modifier();
        }

        if is_key_pressed(KeyCode::Space) {
            if self.menu_cursor == 3 {
                self.start_run();
            } else {
                self.toggle_selected_modifier();
            }
        } else if is_key_pressed(KeyCode::Enter) {
            self.start_run();
        }
    }

    fn handle_direction_input(&mut self) {
        if is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::W) {
            self.snake.queue_direction(Direction::Up);
        }
        if is_key_pressed(KeyCode::Down) || is_key_pressed(KeyCode::S) {
            self.snake.queue_direction(Direction::Down);
        }
        if is_key_pressed(KeyCode::Left) || is_key_pressed(KeyCode::A) {
            self.snake.queue_direction(Direction::Left);
        }
        if is_key_pressed(KeyCode::Right) || is_key_pressed(KeyCode::D) {
            self.snake.queue_direction(Direction::Right);
        }
    }

    fn update_playing(&mut self, dt: f32) {
        if is_key_pressed(KeyCode::P) {
            self.phase = SnakePhase::Paused;
            return;
        }

        self.handle_direction_input();

        if self.chili_boost_timer > 0.0 {
            self.chili_boost_timer = (self.chili_boost_timer - dt).max(0.0);
        }
        if self.status_flash_timer > 0.0 {
            self.status_flash_timer = (self.status_flash_timer - dt).max(0.0);
        }

        self.fruits.update_timers(dt, &self.snake, self.modifiers);

        let speed_mult = if self.chili_boost_timer > 0.0 {
            CHILI_SPEED_MULTIPLIER
        } else {
            1.0
        };
        let tick_interval = BASE_TICK_INTERVAL / speed_mult;

        self.tick_accumulator += dt;
        while self.tick_accumulator >= tick_interval {
            self.tick_accumulator -= tick_interval;

            match self.snake.step(self.modifiers.wall_mode) {
                StepOutcome::Collided => {
                    if self.score > self.high_score {
                        self.high_score = self.score;
                    }
                    self.phase = SnakePhase::GameOver;
                    return;
                }
                StepOutcome::Moved(head) => {
                    match self.fruits.check_eat(head, &self.snake, self.modifiers) {
                        EatOutcome::None => {}
                        EatOutcome::RedApple => {
                            self.snake.add_growth(RED_APPLE_GROWTH);
                            self.score = self.score.saturating_add(RED_APPLE_POINTS);
                        }
                        EatOutcome::GoldenApple => {
                            self.snake.add_growth(GOLDEN_APPLE_GROWTH);
                            self.score = self.score.saturating_add(GOLDEN_APPLE_POINTS);
                            self.status_flash = "GOLDEN +50!";
                            self.status_flash_timer = 1.6;
                        }
                        EatOutcome::ChiliPepper => {
                            self.score = self.score.saturating_add(CHILI_PEPPER_POINTS);
                            self.chili_boost_timer = CHILI_BOOST_DURATION;
                            self.status_flash = "CHILI 1.8x!";
                            self.status_flash_timer = 1.6;
                        }
                    }

                    if self.score > self.high_score {
                        self.high_score = self.score;
                    }

                    if self.snake.is_full() {
                        self.phase = SnakePhase::Victory;
                        return;
                    }
                }
            }
        }
    }

    fn draw_centered_text(text: &str, y: f32, font_size: f32, color: Color) {
        let dims = measure_text(text, None, font_size as u16, 1.0);
        draw_text(text, (VIRTUAL_WIDTH - dims.width) * 0.5, y, font_size, color);
    }

    fn draw_centered_in_board(text: &str, y: f32, font_size: f32, color: Color) {
        let dims = measure_text(text, None, font_size as u16, 1.0);
        draw_text(
            text,
            BOARD_X + (BOARD_PIXEL_SIZE - dims.width) * 0.5,
            y,
            font_size,
            color,
        );
    }

    fn draw_red_apple(cx: f32, cy: f32, radius: f32) {
        draw_circle(cx, cy + 1.5, radius, Color::new(0.94, 0.18, 0.22, 1.0));
        draw_circle(
            cx - radius * 0.3,
            cy - radius * 0.15,
            radius * 0.28,
            Color::new(1.0, 0.55, 0.55, 0.85),
        );
        draw_rectangle(
            cx - 1.5,
            cy - radius - 2.0,
            3.0,
            5.0,
            Color::new(0.54, 0.34, 0.16, 1.0),
        );
        draw_rectangle(cx + 1.5, cy - radius - 2.0, 5.0, 3.0, ACCENT_GREEN);
    }

    fn draw_golden_apple(cx: f32, cy: f32, radius: f32, pulse: f32) {
        let glow_r = radius + 2.5 + pulse * 2.5;
        draw_circle(
            cx,
            cy + 1.0,
            glow_r,
            Color::new(1.0, 0.84, 0.18, 0.22 + 0.15 * pulse),
        );
        draw_circle(cx, cy + 1.5, radius, ACCENT_GOLD);
        draw_circle(
            cx - radius * 0.3,
            cy - radius * 0.2,
            radius * 0.32,
            Color::new(1.0, 0.98, 0.78, 0.95),
        );
        draw_rectangle(
            cx - 1.5,
            cy - radius - 2.0,
            3.0,
            5.0,
            Color::new(0.65, 0.44, 0.12, 1.0),
        );
        draw_rectangle(cx + 1.5, cy - radius - 2.0, 5.0, 3.0, ACCENT_GREEN);
    }

    fn draw_chili_pepper(cx: f32, cy: f32, radius: f32, pulse: f32) {
        let glow_r = radius + 2.5 + pulse * 2.5;
        draw_circle(
            cx,
            cy,
            glow_r,
            Color::new(1.0, 0.35, 0.10, 0.24 + 0.16 * pulse),
        );
        // Tapered fiery chili body built from 3 overlapping circles
        draw_circle(cx - radius * 0.25, cy - radius * 0.1, radius * 0.65, ACCENT_CHILI);
        draw_circle(cx + radius * 0.15, cy + radius * 0.2, radius * 0.52, Color::new(0.95, 0.22, 0.10, 1.0));
        draw_circle(cx + radius * 0.55, cy + radius * 0.45, radius * 0.30, Color::new(1.0, 0.55, 0.15, 1.0));
        // Green stem
        draw_rectangle(
            cx - radius * 0.75,
            cy - radius * 0.75,
            radius * 0.65,
            3.5,
            ACCENT_GREEN,
        );
    }

    fn draw_modifier_select(&self) {
        clear_background(BG_COLOR);

        // Outer cabinet border
        draw_rectangle_lines(
            24.0,
            24.0,
            VIRTUAL_WIDTH - 48.0,
            VIRTUAL_HEIGHT - 48.0,
            2.5,
            BORDER_DIM,
        );

        Self::draw_centered_text("SNAKE", 88.0, 54.0, ACCENT_GREEN);
        Self::draw_centered_text(
            "PRE-GAME MODIFIER CONFIGURATION",
            122.0,
            20.0,
            ACCENT_GOLD,
        );
        draw_line(110.0, 142.0, VIRTUAL_WIDTH - 110.0, 142.0, 1.5, BORDER_DIM);

        let pulse = (self.elapsed * 5.0).sin() * 0.5 + 0.5;
        let row_x = 120.0;
        let row_w = VIRTUAL_WIDTH - 240.0;
        let row_h = 84.0;
        let row_gap = 16.0;
        let start_y = 166.0;

        let special_desc = if self.modifiers.special_items {
            "Golden Apple (+3 len, +50pt, 7s) & Chili Pepper (1.8x speed, 5s)."
        } else {
            "Only standard Red Apples spawn on the grid."
        };

        let rows: [(&str, &str, &str, Color); 3] = [
            (
                "WALL MODE",
                self.modifiers.wall_mode.label(),
                self.modifiers.wall_mode.description(),
                match self.modifiers.wall_mode {
                    WallMode::Classic => ACCENT_CLASSIC,
                    WallMode::Portal => ACCENT_PORTAL,
                },
            ),
            (
                "FRUIT MULTIPLIER",
                self.modifiers.fruit_multiplier.label(),
                self.modifiers.fruit_multiplier.description(),
                ACCENT_GREEN,
            ),
            (
                "SPECIAL ITEMS",
                self.modifiers.special_label(),
                special_desc,
                if self.modifiers.special_items {
                    ACCENT_GOLD
                } else {
                    TEXT_MUTED
                },
            ),
        ];

        for (i, (title, value, desc, accent)) in rows.iter().enumerate() {
            let y = start_y + i as f32 * (row_h + row_gap);
            let selected = self.menu_cursor == i;
            let bg = if selected { PANEL_SELECTED_BG } else { PANEL_BG };

            draw_rectangle(row_x, y, row_w, row_h, bg);
            draw_rectangle(row_x, y, 7.0, row_h, *accent);

            if selected {
                let border = Color::new(accent.r, accent.g, accent.b, 0.55 + 0.45 * pulse);
                draw_rectangle_lines(row_x, y, row_w, row_h, 3.0, border);
                draw_text(">", row_x - 30.0, y + 52.0, 34.0, ACCENT_GOLD);
            } else {
                draw_rectangle_lines(row_x, y, row_w, row_h, 1.5, BORDER_DIM);
            }

            draw_text(title, row_x + 24.0, y + 36.0, 24.0, TEXT_PRIMARY);
            draw_text(desc, row_x + 24.0, y + 64.0, 16.0, TEXT_MUTED);

            // Value toggle badge on the right
            let val_dims = measure_text(value, None, 22, 1.0);
            let badge_w = val_dims.width + 48.0;
            let badge_x = row_x + row_w - badge_w - 20.0;
            let badge_y = y + 20.0;
            draw_rectangle(
                badge_x,
                badge_y,
                badge_w,
                42.0,
                Color::new(accent.r, accent.g, accent.b, 0.16),
            );
            draw_rectangle_lines(badge_x, badge_y, badge_w, 42.0, 1.5, *accent);
            draw_text("<", badge_x + 10.0, badge_y + 28.0, 22.0, TEXT_MUTED);
            draw_text(value, badge_x + 24.0, badge_y + 28.0, 22.0, *accent);
            draw_text(
                ">",
                badge_x + badge_w - 20.0,
                badge_y + 28.0,
                22.0,
                TEXT_MUTED,
            );
        }

        // Row 3: START GAME button
        let btn_y = start_y + 3.0 * (row_h + row_gap) + 4.0;
        let btn_w = 320.0;
        let btn_h = 56.0;
        let btn_x = (VIRTUAL_WIDTH - btn_w) * 0.5;
        let btn_selected = self.menu_cursor == 3;
        let btn_bg = if btn_selected {
            Color::new(0.14, 0.38, 0.22, 1.0)
        } else {
            Color::new(0.08, 0.20, 0.14, 1.0)
        };
        draw_rectangle(btn_x, btn_y, btn_w, btn_h, btn_bg);
        draw_rectangle_lines(
            btn_x,
            btn_y,
            btn_w,
            btn_h,
            if btn_selected { 3.0 } else { 2.0 },
            if btn_selected {
                Color::new(ACCENT_GREEN.r, ACCENT_GREEN.g, ACCENT_GREEN.b, 0.6 + 0.4 * pulse)
            } else {
                ACCENT_GREEN
            },
        );
        Self::draw_centered_text("START GAME", btn_y + 37.0, 30.0, TEXT_PRIMARY);

        // Fruit legend bar
        let legend_y = 556.0;
        draw_rectangle(row_x, legend_y, row_w, 72.0, PANEL_BG);
        draw_rectangle_lines(row_x, legend_y, row_w, 72.0, 1.5, BORDER_DIM);

        Self::draw_red_apple(row_x + 36.0, legend_y + 36.0, 11.0);
        draw_text("RED APPLE", row_x + 58.0, legend_y + 32.0, 17.0, TEXT_PRIMARY);
        draw_text("+1 LEN  +10 PTS", row_x + 58.0, legend_y + 54.0, 15.0, TEXT_MUTED);

        let mid_x = row_x + 242.0;
        Self::draw_golden_apple(mid_x + 22.0, legend_y + 36.0, 11.0, pulse);
        draw_text("GOLDEN APPLE", mid_x + 44.0, legend_y + 32.0, 17.0, ACCENT_GOLD);
        draw_text("+3 LEN  +50 PTS (7s)", mid_x + 44.0, legend_y + 54.0, 15.0, TEXT_MUTED);

        let right_x = row_x + 492.0;
        Self::draw_chili_pepper(right_x + 22.0, legend_y + 36.0, 11.0, pulse);
        draw_text("CHILI PEPPER", right_x + 44.0, legend_y + 32.0, 17.0, ACCENT_CHILI);
        draw_text("1.8x SPEED (5s)", right_x + 44.0, legend_y + 54.0, 15.0, TEXT_MUTED);

        // Controls footer
        Self::draw_centered_text(
            "[UP/DOWN] NAVIGATE   [LEFT/RIGHT/SPACE] TOGGLE   [ENTER] START   [ESC] MENU",
            668.0,
            18.0,
            TEXT_MUTED,
        );
    }

    fn draw_panel(x: f32, y: f32, w: f32, h: f32, title: &str, title_color: Color) {
        draw_rectangle(x, y, w, h, PANEL_BG);
        draw_rectangle_lines(x, y, w, h, 2.0, BORDER_DIM);
        draw_text(title, x + 14.0, y + 26.0, 19.0, title_color);
        draw_line(x + 10.0, y + 34.0, x + w - 10.0, y + 34.0, 1.0, BORDER_DIM);
    }

    fn draw_gameplay(&self) {
        clear_background(BG_COLOR);

        let pulse = (self.elapsed * 6.0).sin() * 0.5 + 0.5;

        // --- 20x20 CHECKERBOARD ARENA ---
        for row in 0..GRID_SIZE {
            for col in 0..GRID_SIZE {
                let color = if (row + col) % 2 == 0 {
                    GRID_LIGHT
                } else {
                    GRID_DARK
                };
                draw_rectangle(
                    BOARD_X + col as f32 * CELL_SIZE,
                    BOARD_Y + row as f32 * CELL_SIZE,
                    CELL_SIZE,
                    CELL_SIZE,
                    color,
                );
            }
        }

        // Standard Red Apples
        for fruit in self.fruits.standard_fruits().iter().flatten() {
            let cx = BOARD_X + (fruit.0 as f32 + 0.5) * CELL_SIZE;
            let cy = BOARD_Y + (fruit.1 as f32 + 0.5) * CELL_SIZE;
            Self::draw_red_apple(cx, cy, CELL_SIZE * 0.36);
        }

        // Timed Special Fruit
        if let Some(special) = self.fruits.special_fruit() {
            let cx = BOARD_X + (special.pos.0 as f32 + 0.5) * CELL_SIZE;
            let cy = BOARD_Y + (special.pos.1 as f32 + 0.5) * CELL_SIZE;
            match special.kind {
                SpecialKind::GoldenApple => Self::draw_golden_apple(cx, cy, CELL_SIZE * 0.38, pulse),
                SpecialKind::ChiliPepper => Self::draw_chili_pepper(cx, cy, CELL_SIZE * 0.36, pulse),
            }
            // Mini countdown bar right under the special fruit cell
            let frac = (special.remaining / special.max_duration).clamp(0.0, 1.0);
            let bar_x = BOARD_X + special.pos.0 as f32 * CELL_SIZE + 3.0;
            let bar_y = BOARD_Y + (special.pos.1 as f32 + 1.0) * CELL_SIZE - 4.0;
            let bar_color = match special.kind {
                SpecialKind::GoldenApple => ACCENT_GOLD,
                SpecialKind::ChiliPepper => ACCENT_CHILI,
            };
            draw_rectangle(bar_x, bar_y, (CELL_SIZE - 6.0) * frac, 3.0, bar_color);
        }

        // Snake body segments (draw tail-to-head so head stays crisp on top)
        let body_len = self.snake.len().max(1) as f32;
        let chili_active = self.chili_boost_timer > 0.0;

        for (rev_idx, &(sx, sy)) in self.snake.body().iter().rev().enumerate() {
            let idx_from_head = self.snake.len().saturating_sub(1 + rev_idx);
            let t = idx_from_head as f32 / body_len;
            let px = BOARD_X + sx as f32 * CELL_SIZE;
            let py = BOARD_Y + sy as f32 * CELL_SIZE;

            let seg_color = if chili_active {
                Color::new(
                    0.98 - t * 0.25,
                    0.44 + t * 0.20,
                    0.12 + t * 0.08,
                    1.0,
                )
            } else if idx_from_head == 0 {
                Color::new(0.32, 0.98, 0.50, 1.0)
            } else {
                Color::new(
                    0.14 + (1.0 - t) * 0.12,
                    0.56 + (1.0 - t) * 0.32,
                    0.26 + (1.0 - t) * 0.14,
                    1.0,
                )
            };

            let pad = if idx_from_head == 0 { 1.5 } else { 2.5 };
            draw_rectangle(
                px + pad,
                py + pad,
                CELL_SIZE - pad * 2.0,
                CELL_SIZE - pad * 2.0,
                seg_color,
            );

            // Subtle inner highlight
            draw_rectangle(
                px + pad + 2.0,
                py + pad + 2.0,
                CELL_SIZE - pad * 2.0 - 4.0,
                3.0,
                Color::new(1.0, 1.0, 1.0, 0.16),
            );

            // Directional eyes on head
            if idx_from_head == 0 {
                let (e1, e2) = match self.snake.direction() {
                    Direction::Right => ((20.0, 9.0), (20.0, 21.0)),
                    Direction::Left => ((10.0, 9.0), (10.0, 21.0)),
                    Direction::Up => ((9.0, 10.0), (21.0, 10.0)),
                    Direction::Down => ((9.0, 20.0), (21.0, 20.0)),
                };
                draw_circle(px + e1.0, py + e1.1, 3.0, BG_COLOR);
                draw_circle(px + e2.0, py + e2.1, 3.0, BG_COLOR);
            }
        }

        // Arena border reflects active Wall Mode (Portal glows violet, Classic solid red)
        let border_color = match self.modifiers.wall_mode {
            WallMode::Classic => ACCENT_CLASSIC,
            WallMode::Portal => Color::new(
                ACCENT_PORTAL.r,
                ACCENT_PORTAL.g,
                ACCENT_PORTAL.b,
                0.65 + 0.35 * pulse,
            ),
        };
        draw_rectangle_lines(
            BOARD_X - 2.0,
            BOARD_Y - 2.0,
            BOARD_PIXEL_SIZE + 4.0,
            BOARD_PIXEL_SIZE + 4.0,
            3.0,
            border_color,
        );

        // --- RIGHT HUD SIDEBAR ---
        let hud_x = BOARD_X + BOARD_PIXEL_SIZE + 32.0; // 688.0
        let hud_w = VIRTUAL_WIDTH - hud_x - 36.0;      // 236.0

        // Score & Length panel
        Self::draw_panel(hud_x, BOARD_Y, hud_w, 198.0, "SNAKE HUD", ACCENT_GREEN);
        let score_buf = NumBuf::from_u32(self.score);
        let high_buf = NumBuf::from_u32(self.high_score);
        let len_buf = NumBuf::from_u32(self.snake.len() as u32);

        draw_text("SCORE", hud_x + 14.0, BOARD_Y + 58.0, 16.0, TEXT_MUTED);
        draw_text(score_buf.as_str(), hud_x + 14.0, BOARD_Y + 86.0, 30.0, ACCENT_GOLD);

        draw_text("BEST", hud_x + 14.0, BOARD_Y + 116.0, 16.0, TEXT_MUTED);
        draw_text(high_buf.as_str(), hud_x + 14.0, BOARD_Y + 142.0, 24.0, TEXT_PRIMARY);

        draw_text("LENGTH", hud_x + 124.0, BOARD_Y + 116.0, 16.0, TEXT_MUTED);
        draw_text(len_buf.as_str(), hud_x + 124.0, BOARD_Y + 142.0, 24.0, ACCENT_GREEN);

        if self.status_flash_timer > 0.0 && !self.status_flash.is_empty() {
            draw_text(self.status_flash, hud_x + 14.0, BOARD_Y + 182.0, 20.0, ACCENT_GOLD);
        }

        // Active Modifiers panel
        let mod_y = BOARD_Y + 216.0;
        Self::draw_panel(hud_x, mod_y, hud_w, 128.0, "MODIFIERS", ACCENT_GOLD);
        let wall_color = match self.modifiers.wall_mode {
            WallMode::Classic => ACCENT_CLASSIC,
            WallMode::Portal => ACCENT_PORTAL,
        };
        draw_text("WALLS:", hud_x + 14.0, mod_y + 60.0, 16.0, TEXT_MUTED);
        draw_text(self.modifiers.wall_mode.label(), hud_x + 92.0, mod_y + 60.0, 16.0, wall_color);

        draw_text("FRUIT:", hud_x + 14.0, mod_y + 86.0, 16.0, TEXT_MUTED);
        draw_text(
            self.modifiers.fruit_multiplier.label(),
            hud_x + 92.0,
            mod_y + 86.0,
            16.0,
            ACCENT_GREEN,
        );

        draw_text("ITEMS:", hud_x + 14.0, mod_y + 112.0, 16.0, TEXT_MUTED);
        draw_text(
            self.modifiers.special_label(),
            hud_x + 92.0,
            mod_y + 112.0,
            16.0,
            if self.modifiers.special_items {
                ACCENT_GOLD
            } else {
                TEXT_MUTED
            },
        );

        // Active Timers panel (Chili Boost + Special Fruit Countdown)
        let timer_y = mod_y + 146.0;
        Self::draw_panel(hud_x, timer_y, hud_w, 124.0, "TIMERS", ACCENT_CHILI);
        let bar_w = hud_w - 28.0;

        if self.chili_boost_timer > 0.0 {
            let chili_buf = NumBuf::from_tenths_sec(self.chili_boost_timer);
            let frac = (self.chili_boost_timer / CHILI_BOOST_DURATION).clamp(0.0, 1.0);
            draw_text("CHILI 1.8x", hud_x + 14.0, timer_y + 58.0, 16.0, ACCENT_CHILI);
            draw_text(chili_buf.as_str(), hud_x + hud_w - 58.0, timer_y + 58.0, 16.0, ACCENT_CHILI);
            draw_rectangle(hud_x + 14.0, timer_y + 64.0, bar_w, 8.0, GRID_DARK);
            draw_rectangle(hud_x + 14.0, timer_y + 64.0, bar_w * frac, 8.0, ACCENT_CHILI);
        } else {
            draw_text("SPEED: NORMAL (1.0x)", hud_x + 14.0, timer_y + 62.0, 15.0, TEXT_MUTED);
        }

        if let Some(special) = self.fruits.special_fruit() {
            let sp_buf = NumBuf::from_tenths_sec(special.remaining);
            let frac = (special.remaining / special.max_duration).clamp(0.0, 1.0);
            let (label, color) = match special.kind {
                SpecialKind::GoldenApple => ("GOLDEN APPLE", ACCENT_GOLD),
                SpecialKind::ChiliPepper => ("CHILI ON GRID", ACCENT_CHILI),
            };
            draw_text(label, hud_x + 14.0, timer_y + 98.0, 16.0, color);
            draw_text(sp_buf.as_str(), hud_x + hud_w - 58.0, timer_y + 98.0, 16.0, color);
            draw_rectangle(hud_x + 14.0, timer_y + 104.0, bar_w, 8.0, GRID_DARK);
            draw_rectangle(hud_x + 14.0, timer_y + 104.0, bar_w * frac, 8.0, color);
        } else if self.modifiers.special_items {
            draw_text("SPECIAL: SPAWNING...", hud_x + 14.0, timer_y + 102.0, 15.0, TEXT_MUTED);
        } else {
            draw_text("SPECIALS: OFF", hud_x + 14.0, timer_y + 102.0, 15.0, TEXT_MUTED);
        }

        // Controls panel
        let ctrl_y = timer_y + 142.0;
        Self::draw_panel(hud_x, ctrl_y, hud_w, 96.0, "CONTROLS", TEXT_PRIMARY);
        draw_text("ARROWS / WASD : TURN", hud_x + 14.0, ctrl_y + 56.0, 15.0, TEXT_MUTED);
        draw_text("P: PAUSE   ESC: MENU", hud_x + 14.0, ctrl_y + 78.0, 15.0, TEXT_MUTED);

        // --- OVERLAYS ---
        match self.phase {
            SnakePhase::Paused => {
                draw_rectangle(
                    BOARD_X,
                    BOARD_Y,
                    BOARD_PIXEL_SIZE,
                    BOARD_PIXEL_SIZE,
                    Color::new(0.02, 0.03, 0.06, 0.78),
                );
                Self::draw_centered_in_board("PAUSED", BOARD_Y + 265.0, 46.0, ACCENT_GREEN);
                Self::draw_centered_in_board("PRESS [P] TO RESUME", BOARD_Y + 318.0, 22.0, TEXT_PRIMARY);
                Self::draw_centered_in_board("PRESS [M] FOR MODIFIERS", BOARD_Y + 352.0, 20.0, ACCENT_GOLD);
            }
            SnakePhase::GameOver => {
                draw_rectangle(
                    BOARD_X,
                    BOARD_Y,
                    BOARD_PIXEL_SIZE,
                    BOARD_PIXEL_SIZE,
                    Color::new(0.06, 0.02, 0.03, 0.84),
                );
                Self::draw_centered_in_board("GAME OVER", BOARD_Y + 246.0, 48.0, ACCENT_CLASSIC);
                Self::draw_centered_in_board(
                    "PRESS [R] OR [ENTER] TO RETRY",
                    BOARD_Y + 306.0,
                    22.0,
                    ACCENT_GOLD,
                );
                Self::draw_centered_in_board(
                    "PRESS [M] TO CHANGE MODIFIERS",
                    BOARD_Y + 340.0,
                    20.0,
                    TEXT_PRIMARY,
                );
                Self::draw_centered_in_board(
                    "[ESC] RETURN TO HUB MENU",
                    BOARD_Y + 382.0,
                    18.0,
                    TEXT_MUTED,
                );
            }
            SnakePhase::Victory => {
                draw_rectangle(
                    BOARD_X,
                    BOARD_Y,
                    BOARD_PIXEL_SIZE,
                    BOARD_PIXEL_SIZE,
                    Color::new(0.02, 0.07, 0.04, 0.84),
                );
                Self::draw_centered_in_board("PERFECT CLEAR!", BOARD_Y + 246.0, 46.0, ACCENT_GOLD);
                Self::draw_centered_in_board(
                    "PRESS [R] OR [ENTER] TO PLAY AGAIN",
                    BOARD_Y + 306.0,
                    22.0,
                    TEXT_PRIMARY,
                );
                Self::draw_centered_in_board(
                    "PRESS [M] TO CHANGE MODIFIERS",
                    BOARD_Y + 340.0,
                    20.0,
                    ACCENT_GREEN,
                );
            }
            SnakePhase::Playing | SnakePhase::ModifierSelect => {}
        }
    }
}

impl Default for SnakeScene {
    fn default() -> Self {
        Self::new()
    }
}

impl GameScene for SnakeScene {
    fn update(&mut self) -> SceneAction {
        let dt = get_frame_time().min(0.1);
        self.elapsed += dt;

        match self.phase {
            SnakePhase::ModifierSelect => self.update_modifier_select(),
            SnakePhase::Playing => self.update_playing(dt),
            SnakePhase::Paused => {
                if is_key_pressed(KeyCode::P) {
                    self.phase = SnakePhase::Playing;
                } else if is_key_pressed(KeyCode::M) {
                    self.phase = SnakePhase::ModifierSelect;
                }
            }
            SnakePhase::GameOver | SnakePhase::Victory => {
                if is_key_pressed(KeyCode::R) || is_key_pressed(KeyCode::Enter) {
                    self.start_run();
                } else if is_key_pressed(KeyCode::M) {
                    self.phase = SnakePhase::ModifierSelect;
                }
            }
        }

        SceneAction::None
    }

    fn draw(&self) {
        match self.phase {
            SnakePhase::ModifierSelect => self.draw_modifier_select(),
            SnakePhase::Playing
            | SnakePhase::Paused
            | SnakePhase::GameOver
            | SnakePhase::Victory => self.draw_gameplay(),
        }
    }
}
