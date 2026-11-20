mod core;
mod games;
mod ui;

use macroquad::prelude::*;

use crate::core::input;
use crate::core::scene::{GameScene, SceneAction, SceneId};
use crate::core::{VIRTUAL_HEIGHT, VIRTUAL_WIDTH};
use crate::games::mario::MarioScene;
use crate::games::snake::SnakeScene;
use crate::games::tetris::TetrisScene;
use crate::ui::menu::MenuScene;

fn window_conf() -> Conf {
    Conf {
        window_title: "Retro Arcade Hub".to_owned(),
        window_width: VIRTUAL_WIDTH as i32,
        window_height: VIRTUAL_HEIGHT as i32,
        window_resizable: true,
        ..Default::default()
    }
}

/// Instantiates a fresh scene for the requested `SceneId`.
fn create_scene(id: SceneId) -> Box<dyn GameScene> {
    match id {
        SceneId::Menu => Box::new(MenuScene::new()),
        SceneId::Tetris => Box::new(TetrisScene::new()),
        SceneId::Snake => Box::new(SnakeScene::new()),
        SceneId::Mario => Box::new(MarioScene::new()),
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    // Allocate the 960x720 virtual canvas once outside the game loop.
    let virtual_canvas = render_target(VIRTUAL_WIDTH as u32, VIRTUAL_HEIGHT as u32);
    virtual_canvas.texture.set_filter(FilterMode::Nearest);

    // Positive Y zoom maps y=0 to texture v=0 (top when blitted to the default screen camera).
    let virtual_camera = Camera2D {
        target: vec2(VIRTUAL_WIDTH * 0.5, VIRTUAL_HEIGHT * 0.5),
        zoom: vec2(2.0 / VIRTUAL_WIDTH, 2.0 / VIRTUAL_HEIGHT),
        render_target: Some(virtual_canvas.clone()),
        ..Default::default()
    };

    let mut active_id = SceneId::Menu;
    let mut active_scene: Box<dyn GameScene> = create_scene(active_id);

    loop {
        // Universal Exit rule: pressing Escape in any scene triggers SceneAction::QuitToMenu.
        let action = if input::is_escape_pressed() {
            SceneAction::QuitToMenu
        } else {
            active_scene.update()
        };

        match action {
            SceneAction::None => {}
            SceneAction::SwitchTo(next_id) => {
                // Dropping the old Box frees the previous scene's memory immediately.
                active_id = next_id;
                active_scene = create_scene(next_id);
            }
            SceneAction::QuitToMenu => {
                if active_id == SceneId::Menu {
                    break;
                }
                active_id = SceneId::Menu;
                active_scene = create_scene(SceneId::Menu);
            }
            SceneAction::QuitApp => {
                break;
            }
        }

        // 1. Render the active scene onto the 960x720 virtual canvas.
        set_camera(&virtual_camera);
        active_scene.draw();

        // 2. Blit the virtual canvas onto the physical window preserving 4:3 aspect ratio
        //    with black letterboxing/pillarboxing.
        set_default_camera();
        clear_background(BLACK);

        let win_w = screen_width();
        let win_h = screen_height();
        let scale = (win_w / VIRTUAL_WIDTH).min(win_h / VIRTUAL_HEIGHT);
        let draw_w = VIRTUAL_WIDTH * scale;
        let draw_h = VIRTUAL_HEIGHT * scale;
        let offset_x = (win_w - draw_w) * 0.5;
        let offset_y = (win_h - draw_h) * 0.5;

        draw_texture_ex(
            &virtual_canvas.texture,
            offset_x,
            offset_y,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(draw_w, draw_h)),
                ..Default::default()
            },
        );

        next_frame().await;
    }
}
