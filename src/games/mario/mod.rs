pub mod bus;
pub mod cartridge;
pub mod cpu;
pub mod joypad;
pub mod ppu;

use macroquad::prelude::*;

use crate::core::scene::{GameScene, SceneAction};
use crate::core::{VIRTUAL_HEIGHT, VIRTUAL_WIDTH};
use crate::games::mario::bus::Bus;
use crate::games::mario::cartridge::{Cartridge, CartridgeError};
use crate::games::mario::cpu::Cpu;
use crate::games::mario::joypad::button;
use crate::games::mario::ppu::{FRAMEBUFFER_SIZE, SCREEN_HEIGHT, SCREEN_WIDTH};

const ROM_PATH: &str = "assets/roms/mario.nes";
const CPU_CYCLES_PER_FRAME: u32 = 29_780;
const NES_FRAME_TIME: f32 = 1.0 / 60.0;

// Exact 4:3 display viewport centered inside the 960x720 virtual canvas (864 / 648 = 4/3).
const VIEWPORT_W: f32 = 864.0;
const VIEWPORT_H: f32 = 648.0;
const VIEWPORT_X: f32 = (VIRTUAL_WIDTH - VIEWPORT_W) * 0.5;
const VIEWPORT_Y: f32 = 20.0;

const BG_COLOR: Color = Color::new(0.045, 0.055, 0.10, 1.0);
const PANEL_BG: Color = Color::new(0.075, 0.095, 0.16, 1.0);
const BORDER_DIM: Color = Color::new(0.24, 0.28, 0.42, 1.0);
const ACCENT_RED: Color = Color::new(0.98, 0.30, 0.26, 1.0);
const ACCENT_GOLD: Color = Color::new(1.0, 0.82, 0.22, 1.0);
const ACCENT_CYAN: Color = Color::new(0.18, 0.88, 0.98, 1.0);
const TEXT_PRIMARY: Color = Color::new(0.95, 0.96, 0.99, 1.0);
const TEXT_MUTED: Color = Color::new(0.60, 0.67, 0.80, 1.0);

/// Self-contained headless NES console instance (CPU + Bus + PPU + NROM Cartridge).
struct NesCore {
    cpu: Cpu,
    bus: Bus,
    cycle_debt: u32,
}

impl NesCore {
    fn new(cartridge: Cartridge) -> Self {
        let mut bus = Bus::new(cartridge);
        let mut cpu = Cpu::new();
        cpu.reset(&mut bus);
        Self {
            cpu,
            bus,
            cycle_debt: 0,
        }
    }

    fn reset(&mut self) {
        self.bus.reset();
        self.cpu.reset(&mut self.bus);
        self.cycle_debt = 0;
    }

    /// Runs one NTSC video frame synchronized to 29,780 CPU cycles.
    fn step_frame(&mut self) {
        let mut ran = self.cycle_debt;
        while ran < CPU_CYCLES_PER_FRAME {
            ran += self.cpu.step(&mut self.bus);
        }
        self.cycle_debt = ran - CPU_CYCLES_PER_FRAME;
    }

    #[inline]
    fn set_buttons(&mut self, mask: u8) {
        self.bus.set_joypad_buttons(mask);
    }

    #[inline]
    fn framebuffer(&self) -> &[u8; FRAMEBUFFER_SIZE] {
        self.bus.framebuffer()
    }
}

enum EmulatorState {
    Running {
        nes: Box<NesCore>,
        texture: Texture2D,
        time_accumulator: f32,
        paused: bool,
    },
    RomError(CartridgeError),
}

/// Host scene integrating the headless NES core with Macroquad's 960x720 virtual canvas.
pub struct MarioScene {
    state: EmulatorState,
}

impl MarioScene {
    pub fn new() -> Self {
        Self {
            state: Self::try_boot_rom(),
        }
    }

    fn try_boot_rom() -> EmulatorState {
        match Cartridge::from_file(ROM_PATH) {
            Ok(cartridge) => {
                let nes = Box::new(NesCore::new(cartridge));
                let texture = Texture2D::from_rgba8(
                    SCREEN_WIDTH as u16,
                    SCREEN_HEIGHT as u16,
                    nes.framebuffer(),
                );
                texture.set_filter(FilterMode::Nearest);
                EmulatorState::Running {
                    nes,
                    texture,
                    time_accumulator: 0.0,
                    paused: false,
                }
            }
            Err(err) => EmulatorState::RomError(err),
        }
    }

    fn poll_host_joypad() -> u8 {
        let mut mask = 0u8;
        if is_key_down(KeyCode::X) || is_key_down(KeyCode::K) {
            mask |= button::A;
        }
        if is_key_down(KeyCode::Z) || is_key_down(KeyCode::J) {
            mask |= button::B;
        }
        if is_key_down(KeyCode::RightShift) {
            mask |= button::SELECT;
        }
        if is_key_down(KeyCode::Enter) {
            mask |= button::START;
        }
        if is_key_down(KeyCode::Up) {
            mask |= button::UP;
        }
        if is_key_down(KeyCode::Down) {
            mask |= button::DOWN;
        }
        if is_key_down(KeyCode::Left) {
            mask |= button::LEFT;
        }
        if is_key_down(KeyCode::Right) {
            mask |= button::RIGHT;
        }
        mask
    }

    fn draw_centered_text(text: &str, y: f32, font_size: f32, color: Color) {
        let dims = measure_text(text, None, font_size as u16, 1.0);
        draw_text(text, (VIRTUAL_WIDTH - dims.width) * 0.5, y, font_size, color);
    }

    fn draw_missing_rom_banner(err: CartridgeError) {
        clear_background(BG_COLOR);

        // Outer bezel frame
        draw_rectangle_lines(
            28.0,
            28.0,
            VIRTUAL_WIDTH - 56.0,
            VIRTUAL_HEIGHT - 56.0,
            2.5,
            BORDER_DIM,
        );

        Self::draw_centered_text("SUPER MARIO BROS", 104.0, 50.0, ACCENT_RED);
        Self::draw_centered_text(
            "EMBEDDED HEADLESS NES CORE  |  RICOH 2A03 + 2C02 PPU (MAPPER 0 / NROM)",
            138.0,
            18.0,
            ACCENT_GOLD,
        );

        // Diagnostic banner card
        let box_x = 96.0;
        let box_y = 176.0;
        let box_w = VIRTUAL_WIDTH - 192.0;
        let box_h = 248.0;
        draw_rectangle(box_x, box_y, box_w, box_h, PANEL_BG);
        draw_rectangle(box_x, box_y, 8.0, box_h, ACCENT_RED);
        draw_rectangle_lines(box_x, box_y, box_w, box_h, 2.0, ACCENT_RED);

        draw_text("CARTRIDGE SLOT EMPTY / ROM ERROR", box_x + 28.0, box_y + 44.0, 26.0, ACCENT_RED);
        draw_text(err.message(), box_x + 28.0, box_y + 82.0, 21.0, TEXT_PRIMARY);

        draw_line(
            box_x + 28.0,
            box_y + 104.0,
            box_x + box_w - 28.0,
            box_y + 104.0,
            1.0,
            BORDER_DIM,
        );

        draw_text("HOW TO INSTALL YOUR ROM:", box_x + 28.0, box_y + 138.0, 20.0, ACCENT_CYAN);
        draw_text(
            "1. Obtain a legal iNES (.nes) NROM Mapper 0 dump of Super Mario Bros.",
            box_x + 28.0,
            box_y + 170.0,
            18.0,
            TEXT_MUTED,
        );
        draw_text(
            "2. Place the file at:  assets/roms/mario.nes",
            box_x + 28.0,
            box_y + 198.0,
            18.0,
            ACCENT_GOLD,
        );
        draw_text(
            "3. Press [R] to retry loading (proprietary *.nes files are Git-ignored).",
            box_x + 28.0,
            box_y + 226.0,
            18.0,
            TEXT_MUTED,
        );

        // Joypad keybinding reference panel
        let pad_y = 456.0;
        let pad_h = 152.0;
        draw_rectangle(box_x, pad_y, box_w, pad_h, PANEL_BG);
        draw_rectangle_lines(box_x, pad_y, box_w, pad_h, 1.5, BORDER_DIM);
        draw_text("NES CONTROLLER BINDINGS", box_x + 28.0, pad_y + 34.0, 20.0, ACCENT_CYAN);

        draw_text("ARROW KEYS   : D-PAD (UP / DOWN / LEFT / RIGHT)", box_x + 28.0, pad_y + 68.0, 18.0, TEXT_PRIMARY);
        draw_text("Z / J        : NES B (RUN / FIREBALL)", box_x + 28.0, pad_y + 96.0, 18.0, TEXT_PRIMARY);
        draw_text("X / K        : NES A (JUMP)", box_x + 28.0, pad_y + 124.0, 18.0, TEXT_PRIMARY);

        let col2_x = box_x + 448.0;
        draw_text("RIGHT SHIFT  : SELECT", col2_x, pad_y + 96.0, 18.0, TEXT_PRIMARY);
        draw_text("ENTER        : START", col2_x, pad_y + 124.0, 18.0, TEXT_PRIMARY);

        Self::draw_centered_text(
            "[R] RETRY LOAD ROM      [ESC] RETURN TO HUB MENU",
            660.0,
            20.0,
            ACCENT_GOLD,
        );
    }
}

impl Default for MarioScene {
    fn default() -> Self {
        Self::new()
    }
}

impl GameScene for MarioScene {
    fn update(&mut self) -> SceneAction {
        match &mut self.state {
            EmulatorState::Running {
                nes,
                texture,
                time_accumulator,
                paused,
            } => {
                if is_key_pressed(KeyCode::P) {
                    *paused = !*paused;
                }
                if is_key_pressed(KeyCode::F5) {
                    nes.reset();
                    *time_accumulator = 0.0;
                    *paused = false;
                }
                if *paused {
                    return SceneAction::None;
                }

                nes.set_buttons(Self::poll_host_joypad());

                // Cap frame dt to prevent spiral-of-death on window drag/hitches
                let dt = get_frame_time().min(NES_FRAME_TIME * 3.0);
                *time_accumulator += dt;

                let mut stepped = false;
                while *time_accumulator >= NES_FRAME_TIME {
                    *time_accumulator -= NES_FRAME_TIME;
                    nes.step_frame();
                    stepped = true;
                }

                if stepped {
                    texture.update_from_bytes(
                        SCREEN_WIDTH as u32,
                        SCREEN_HEIGHT as u32,
                        nes.framebuffer(),
                    );
                }
            }
            EmulatorState::RomError(_) => {
                if is_key_pressed(KeyCode::R) {
                    self.state = Self::try_boot_rom();
                }
            }
        }

        SceneAction::None
    }

    fn draw(&self) {
        match &self.state {
            EmulatorState::Running {
                texture, paused, ..
            } => {
                clear_background(BG_COLOR);

                // Blit 256x240 framebuffer scaled cleanly at exact 4:3 aspect ratio
                draw_texture_ex(
                    texture,
                    VIEWPORT_X,
                    VIEWPORT_Y,
                    WHITE,
                    DrawTextureParams {
                        dest_size: Some(vec2(VIEWPORT_W, VIEWPORT_H)),
                        ..Default::default()
                    },
                );

                // Cabinet bezel around the 4:3 NES screen
                draw_rectangle_lines(
                    VIEWPORT_X - 2.0,
                    VIEWPORT_Y - 2.0,
                    VIEWPORT_W + 4.0,
                    VIEWPORT_H + 4.0,
                    2.5,
                    BORDER_DIM,
                );

                // Compact control bar below viewport
                Self::draw_centered_text(
                    "D-PAD:[ARROWS]  B:[Z/J]  A:[X/K]  SEL:[R-SHIFT]  START:[ENTER]  [P] PAUSE  [F5] RESET  [ESC] MENU",
                    700.0,
                    16.0,
                    TEXT_MUTED,
                );

                if *paused {
                    draw_rectangle(
                        VIEWPORT_X,
                        VIEWPORT_Y,
                        VIEWPORT_W,
                        VIEWPORT_H,
                        Color::new(0.02, 0.03, 0.06, 0.72),
                    );
                    Self::draw_centered_text("EMULATION PAUSED", 344.0, 44.0, ACCENT_GOLD);
                    Self::draw_centered_text("PRESS [P] TO RESUME   |   [F5] HARD RESET", 392.0, 20.0, TEXT_PRIMARY);
                }
            }
            EmulatorState::RomError(err) => {
                Self::draw_missing_rom_banner(*err);
            }
        }
    }
}
