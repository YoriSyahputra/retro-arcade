use macroquad::prelude::*;

use crate::core::input;
use crate::core::scene::{GameScene, SceneAction, SceneId};
use crate::core::{VIRTUAL_HEIGHT, VIRTUAL_WIDTH};

#[derive(Clone, Copy)]
struct GameEntry {
    scene_id: SceneId,
    title: &'static str,
    subtitle: &'static str,
    tag: &'static str,
    accent: Color,
}

const GAMES: [GameEntry; 3] = [
    GameEntry {
        scene_id: SceneId::Tetris,
        title: "TETRIS",
        subtitle: "10x20 MATRIX  |  SRS WALL-KICKS  |  7-BAG RNG",
        tag: "PUZZLE",
        accent: Color::new(0.0, 0.88, 0.96, 1.0),
    },
    GameEntry {
        scene_id: SceneId::Snake,
        title: "SNAKE",
        subtitle: "PORTAL WALLS  |  GOLDEN APPLES  |  CHILI BOOST",
        tag: "ARCADE",
        accent: Color::new(0.24, 0.90, 0.42, 1.0),
    },
    GameEntry {
        scene_id: SceneId::Mario,
        title: "SUPER MARIO BROS",
        subtitle: "EMBEDDED HEADLESS NES CORE  |  RICOH 2A03",
        tag: "NES ROM",
        accent: Color::new(0.98, 0.32, 0.28, 1.0),
    },
];

const BG_COLOR: Color = Color::new(0.045, 0.055, 0.10, 1.0);
const PANEL_BG: Color = Color::new(0.08, 0.10, 0.17, 1.0);
const PANEL_SELECTED_BG: Color = Color::new(0.12, 0.15, 0.26, 1.0);
const BORDER_DIM: Color = Color::new(0.22, 0.27, 0.40, 1.0);
const TEXT_PRIMARY: Color = Color::new(0.95, 0.96, 0.99, 1.0);
const TEXT_MUTED: Color = Color::new(0.58, 0.65, 0.78, 1.0);
const NEON_GOLD: Color = Color::new(1.0, 0.82, 0.22, 1.0);
const NEON_CYAN: Color = Color::new(0.18, 0.88, 0.98, 1.0);

/// Retro arcade hub launcher menu rendered on the 960x720 virtual canvas.
pub struct MenuScene {
    selected: usize,
    elapsed: f32,
}

impl MenuScene {
    pub fn new() -> Self {
        Self {
            selected: 0,
            elapsed: 0.0,
        }
    }

    fn draw_centered_text(text: &str, y: f32, font_size: f32, color: Color) {
        let dims = measure_text(text, None, font_size as u16, 1.0);
        let x = (VIRTUAL_WIDTH - dims.width) * 0.5;
        draw_text(text, x, y, font_size, color);
    }

    fn draw_preview_icon(scene_id: SceneId, x: f32, y: f32, size: f32) {
        // Icon frame background
        draw_rectangle(x, y, size, size, Color::new(0.04, 0.05, 0.09, 1.0));
        draw_rectangle_lines(x, y, size, size, 2.0, BORDER_DIM);

        let cell = size / 6.0;
        match scene_id {
            SceneId::Tetris => {
                // Mini T-piece (purple) and L-piece (orange) + I-piece (cyan)
                let purple = Color::new(0.68, 0.28, 0.96, 1.0);
                let cyan = Color::new(0.0, 0.88, 0.96, 1.0);
                let yellow = Color::new(1.0, 0.84, 0.16, 1.0);
                // T-piece near top
                draw_rectangle(x + cell * 2.0, y + cell * 1.0, cell - 2.0, cell - 2.0, purple);
                draw_rectangle(x + cell * 1.0, y + cell * 2.0, cell - 2.0, cell - 2.0, purple);
                draw_rectangle(x + cell * 2.0, y + cell * 2.0, cell - 2.0, cell - 2.0, purple);
                draw_rectangle(x + cell * 3.0, y + cell * 2.0, cell - 2.0, cell - 2.0, purple);
                // O-piece bottom-right
                draw_rectangle(x + cell * 3.5, y + cell * 3.5, cell - 2.0, cell - 2.0, yellow);
                draw_rectangle(x + cell * 4.5, y + cell * 3.5, cell - 2.0, cell - 2.0, yellow);
                // I-piece bottom-left
                draw_rectangle(x + cell * 0.5, y + cell * 4.5, cell - 2.0, cell - 2.0, cyan);
                draw_rectangle(x + cell * 1.5, y + cell * 4.5, cell - 2.0, cell - 2.0, cyan);
                draw_rectangle(x + cell * 2.5, y + cell * 4.5, cell - 2.0, cell - 2.0, cyan);
            }
            SceneId::Snake => {
                let body = Color::new(0.18, 0.78, 0.35, 1.0);
                let head = Color::new(0.36, 0.98, 0.50, 1.0);
                let apple = Color::new(0.95, 0.22, 0.26, 1.0);
                // Snake body segments
                draw_rectangle(x + cell * 1.0, y + cell * 4.0, cell - 2.0, cell - 2.0, body);
                draw_rectangle(x + cell * 2.0, y + cell * 4.0, cell - 2.0, cell - 2.0, body);
                draw_rectangle(x + cell * 2.0, y + cell * 3.0, cell - 2.0, cell - 2.0, body);
                draw_rectangle(x + cell * 2.0, y + cell * 2.0, cell - 2.0, cell - 2.0, body);
                draw_rectangle(x + cell * 3.0, y + cell * 2.0, cell - 2.0, cell - 2.0, head);
                // Red apple
                draw_rectangle(x + cell * 4.2, y + cell * 3.2, cell - 2.0, cell - 2.0, apple);
            }
            SceneId::Mario => {
                // Classic ? block & brick platform
                let gold = Color::new(0.98, 0.72, 0.16, 1.0);
                let brick = Color::new(0.76, 0.32, 0.18, 1.0);
                let red = Color::new(0.92, 0.20, 0.20, 1.0);
                // Question block
                draw_rectangle(x + cell * 2.0, y + cell * 1.2, cell * 2.0, cell * 2.0, gold);
                draw_rectangle_lines(
                    x + cell * 2.0,
                    y + cell * 1.2,
                    cell * 2.0,
                    cell * 2.0,
                    2.0,
                    Color::new(0.45, 0.25, 0.05, 1.0),
                );
                draw_text("?", x + cell * 2.55, y + cell * 2.85, 24.0, BG_COLOR);
                // Ground bricks
                for i in 0..5 {
                    draw_rectangle(
                        x + cell * (0.5 + i as f32),
                        y + cell * 4.3,
                        cell - 2.0,
                        cell - 2.0,
                        brick,
                    );
                }
                // Mini red cap silhouette
                draw_rectangle(x + cell * 1.0, y + cell * 3.3, cell, cell * 0.8, red);
            }
            SceneId::Menu => {}
        }
    }
}

impl Default for MenuScene {
    fn default() -> Self {
        Self::new()
    }
}

impl GameScene for MenuScene {
    fn update(&mut self) -> SceneAction {
        self.elapsed += get_frame_time();

        if input::is_menu_prev_pressed() {
            self.selected = if self.selected == 0 {
                GAMES.len() - 1
            } else {
                self.selected - 1
            };
        } else if input::is_menu_next_pressed() {
            self.selected = (self.selected + 1) % GAMES.len();
        }

        if input::is_menu_confirm_pressed() {
            return SceneAction::SwitchTo(GAMES[self.selected].scene_id);
        }

        if input::is_menu_quit_pressed() {
            return SceneAction::QuitApp;
        }

        SceneAction::None
    }

    fn draw(&self) {
        clear_background(BG_COLOR);

        // Subtle retro CRT scanlines across the virtual canvas
        let scanline = Color::new(0.0, 0.0, 0.0, 0.16);
        let mut y = 0.0;
        while y < VIRTUAL_HEIGHT {
            draw_line(0.0, y, VIRTUAL_WIDTH, y, 1.0, scanline);
            y += 6.0;
        }

        // Outer arcade bezel border
        draw_rectangle_lines(20.0, 20.0, VIRTUAL_WIDTH - 40.0, VIRTUAL_HEIGHT - 40.0, 3.0, BORDER_DIM);
        draw_rectangle_lines(28.0, 28.0, VIRTUAL_WIDTH - 56.0, VIRTUAL_HEIGHT - 56.0, 1.0, Color::new(0.14, 0.20, 0.34, 1.0));

        // Header banner
        Self::draw_centered_text("RETRO ARCADE HUB", 100.0, 56.0, Color::new(0.0, 0.35, 0.45, 0.9));
        Self::draw_centered_text("RETRO ARCADE HUB", 96.0, 56.0, NEON_CYAN);
        Self::draw_centered_text(
            "NATIVE RUST 2D ARCADE SYSTEM  -  SELECT A CARTRIDGE",
            134.0,
            20.0,
            NEON_GOLD,
        );
        draw_line(110.0, 156.0, VIRTUAL_WIDTH - 110.0, 156.0, 2.0, BORDER_DIM);

        // Game selection cards
        let card_x = 96.0;
        let card_w = VIRTUAL_WIDTH - 192.0;
        let card_h = 128.0;
        let card_gap = 22.0;
        let start_y = 188.0;

        let pulse = (self.elapsed * 5.0).sin() * 0.5 + 0.5;
        let blink_visible = ((self.elapsed * 2.5) as i32) % 2 == 0;

        for (index, game) in GAMES.iter().enumerate() {
            let is_selected = index == self.selected;
            let card_y = start_y + index as f32 * (card_h + card_gap);

            let bg = if is_selected {
                PANEL_SELECTED_BG
            } else {
                PANEL_BG
            };
            draw_rectangle(card_x, card_y, card_w, card_h, bg);

            // Left accent bar
            draw_rectangle(card_x, card_y, 8.0, card_h, game.accent);

            // Card border (pulsing highlight when selected)
            if is_selected {
                let border_color = Color::new(
                    game.accent.r,
                    game.accent.g,
                    game.accent.b,
                    0.55 + 0.45 * pulse,
                );
                draw_rectangle_lines(card_x, card_y, card_w, card_h, 3.0, border_color);
                // Selector arrow to the left of the card
                draw_text(">", card_x - 34.0, card_y + 76.0, 40.0, NEON_GOLD);
            } else {
                draw_rectangle_lines(card_x, card_y, card_w, card_h, 2.0, BORDER_DIM);
            }

            // Miniature pixel preview icon
            Self::draw_preview_icon(game.scene_id, card_x + 26.0, card_y + 20.0, 88.0);

            // Game title & subtitle
            let text_x = card_x + 136.0;
            let title_color = if is_selected { TEXT_PRIMARY } else { Color::new(0.80, 0.84, 0.92, 1.0) };
            draw_text(game.title, text_x, card_y + 56.0, 36.0, title_color);
            draw_text(game.subtitle, text_x, card_y + 88.0, 18.0, TEXT_MUTED);

            // Genre tag badge at top-right of card
            let tag_dims = measure_text(game.tag, None, 16, 1.0);
            let tag_w = tag_dims.width + 18.0;
            let tag_x = card_x + card_w - tag_w - 20.0;
            let tag_y = card_y + 20.0;
            draw_rectangle(
                tag_x,
                tag_y,
                tag_w,
                26.0,
                Color::new(game.accent.r, game.accent.g, game.accent.b, 0.18),
            );
            draw_rectangle_lines(tag_x, tag_y, tag_w, 26.0, 1.5, game.accent);
            draw_text(game.tag, tag_x + 9.0, tag_y + 18.0, 16.0, game.accent);

            // Blinking launch hint on selected card
            if is_selected && blink_visible {
                let hint = "PRESS ENTER";
                let hint_dims = measure_text(hint, None, 18, 1.0);
                draw_text(
                    hint,
                    card_x + card_w - hint_dims.width - 20.0,
                    card_y + card_h - 20.0,
                    18.0,
                    NEON_GOLD,
                );
            }
        }

        // Bottom controls footer bar
        draw_line(110.0, 640.0, VIRTUAL_WIDTH - 110.0, 640.0, 1.5, BORDER_DIM);
        Self::draw_centered_text(
            "[UP / DOWN] SELECT    [ENTER / SPACE] LAUNCH    [Q / ESC] QUIT",
            672.0,
            20.0,
            TEXT_MUTED,
        );
    }
}
