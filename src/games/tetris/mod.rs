use macroquad::prelude::*;

use crate::core::scene::{GameScene, SceneAction};
use crate::core::{VIRTUAL_HEIGHT, VIRTUAL_WIDTH};

/// Placeholder scene for the Tetris module (implemented in Phase 2).
#[derive(Default)]
pub struct TetrisScene;

impl TetrisScene {
    pub fn new() -> Self {
        Self
    }
}

impl GameScene for TetrisScene {
    fn update(&mut self) -> SceneAction {
        SceneAction::None
    }

    fn draw(&self) {
        clear_background(Color::new(0.045, 0.055, 0.10, 1.0));
        draw_rectangle_lines(
            40.0,
            40.0,
            VIRTUAL_WIDTH - 80.0,
            VIRTUAL_HEIGHT - 80.0,
            2.0,
            Color::new(0.0, 0.88, 0.96, 1.0),
        );

        let title = "TETRIS";
        let title_dims = measure_text(title, None, 52, 1.0);
        draw_text(
            title,
            (VIRTUAL_WIDTH - title_dims.width) * 0.5,
            VIRTUAL_HEIGHT * 0.44,
            52.0,
            Color::new(0.0, 0.88, 0.96, 1.0),
        );

        let subtitle = "PLACEHOLDER SCENE - PHASE 2 IMPLEMENTATION";
        let sub_dims = measure_text(subtitle, None, 24, 1.0);
        draw_text(
            subtitle,
            (VIRTUAL_WIDTH - sub_dims.width) * 0.5,
            VIRTUAL_HEIGHT * 0.52,
            24.0,
            Color::new(0.75, 0.80, 0.90, 1.0),
        );

        let hint = "PRESS [ESC] TO RETURN TO MENU";
        let hint_dims = measure_text(hint, None, 22, 1.0);
        draw_text(
            hint,
            (VIRTUAL_WIDTH - hint_dims.width) * 0.5,
            VIRTUAL_HEIGHT * 0.62,
            22.0,
            Color::new(1.0, 0.82, 0.22, 1.0),
        );
    }
}
