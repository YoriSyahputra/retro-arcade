use crate::games::mario::cartridge::{Cartridge, Mirroring};

pub const SCREEN_WIDTH: usize = 256;
pub const SCREEN_HEIGHT: usize = 240;
pub const FRAMEBUFFER_SIZE: usize = SCREEN_WIDTH * SCREEN_HEIGHT * 4;

const DOTS_PER_SCANLINE: u16 = 341;
const SCANLINES_PER_FRAME: u16 = 262;

/// Authentic 64-color NES 2C02 RGB hardware palette.
const NES_PALETTE: [(u8, u8, u8); 64] = [
    (84, 84, 84),    (0, 30, 116),    (8, 16, 144),    (48, 0, 136),
    (68, 0, 100),    (92, 0, 48),     (84, 4, 0),      (60, 24, 0),
    (32, 42, 0),     (8, 58, 0),      (0, 64, 0),      (0, 60, 0),
    (0, 50, 60),     (0, 0, 0),       (0, 0, 0),       (0, 0, 0),
    (152, 150, 152), (8, 76, 196),    (48, 50, 236),   (92, 30, 228),
    (136, 20, 176),  (160, 20, 100),  (152, 34, 32),   (120, 60, 0),
    (84, 90, 0),     (40, 114, 0),    (8, 124, 0),     (0, 118, 40),
    (0, 102, 120),   (0, 0, 0),       (0, 0, 0),       (0, 0, 0),
    (236, 238, 236), (76, 154, 236),  (120, 124, 236), (176, 98, 236),
    (228, 84, 236),  (236, 88, 180),  (236, 106, 100), (212, 136, 32),
    (160, 170, 0),   (116, 196, 0),   (76, 208, 32),   (56, 204, 108),
    (56, 180, 204),  (60, 60, 60),    (0, 0, 0),       (0, 0, 0),
    (236, 238, 236), (168, 204, 236), (188, 188, 236), (212, 178, 236),
    (236, 174, 236), (236, 174, 212), (236, 180, 176), (228, 196, 144),
    (204, 210, 120), (180, 222, 120), (168, 226, 144), (152, 226, 180),
    (160, 214, 228), (160, 162, 160), (0, 0, 0),       (0, 0, 0),
];

/// Emulates the NES 2C02 Picture Processing Unit (256x240 RGBA output).
pub struct Ppu {
    // Internal memories
    vram: [u8; 4096],
    palette_ram: [u8; 32],
    oam: [u8; 256],

    // Registers ($2000-$2007) & Loopy scroll latches
    ctrl: u8,
    mask: u8,
    status: u8,
    oam_addr: u8,
    v: u16,
    t: u16,
    fine_x: u8,
    write_latch: bool,
    data_buffer: u8,
    open_bus: u8,

    // Scanline timing (341 dots x 262 scanlines)
    scanline: u16,
    dot: u16,
    pending_sprite0_hit_x: Option<u16>,
    nmi_pending: bool,

    // 256x240 RGBA framebuffer (boxed once at init; zero loop allocations)
    framebuffer: Box<[u8; FRAMEBUFFER_SIZE]>,
}

impl Ppu {
    pub fn new() -> Self {
        let mut fb = Box::new([0u8; FRAMEBUFFER_SIZE]);
        for px in fb.as_chunks_mut::<4>().0 {
            px[3] = 255;
        }
        Self {
            vram: [0; 4096],
            palette_ram: [0; 32],
            oam: [0; 256],
            ctrl: 0,
            mask: 0,
            status: 0,
            oam_addr: 0,
            v: 0,
            t: 0,
            fine_x: 0,
            write_latch: false,
            data_buffer: 0,
            open_bus: 0,
            scanline: 0,
            dot: 0,
            pending_sprite0_hit_x: None,
            nmi_pending: false,
            framebuffer: fb,
        }
    }

    pub fn reset(&mut self) {
        self.ctrl = 0;
        self.mask = 0;
        self.status = 0;
        self.oam_addr = 0;
        self.v = 0;
        self.t = 0;
        self.fine_x = 0;
        self.write_latch = false;
        self.data_buffer = 0;
        self.open_bus = 0;
        self.scanline = 0;
        self.dot = 0;
        self.pending_sprite0_hit_x = None;
        self.nmi_pending = false;
    }

    #[inline]
    pub fn framebuffer(&self) -> &[u8; FRAMEBUFFER_SIZE] {
        &self.framebuffer
    }

    /// Consumes and returns any pending NMI raised on VBlank entry.
    #[inline]
    pub fn poll_nmi(&mut self) -> bool {
        let triggered = self.nmi_pending;
        self.nmi_pending = false;
        triggered
    }

    /// Writes one byte to OAM during `$4014` DMA transfer.
    #[inline]
    pub fn write_oam_dma_byte(&mut self, data: u8) {
        self.oam[self.oam_addr as usize] = data;
        self.oam_addr = self.oam_addr.wrapping_add(1);
    }

    #[inline]
    fn show_bg(&self) -> bool {
        (self.mask & 0x08) != 0
    }

    #[inline]
    fn show_sprites(&self) -> bool {
        (self.mask & 0x10) != 0
    }

    #[inline]
    fn show_bg_left(&self) -> bool {
        (self.mask & 0x02) != 0
    }

    #[inline]
    fn show_sprites_left(&self) -> bool {
        (self.mask & 0x04) != 0
    }

    #[inline]
    fn rendering_enabled(&self) -> bool {
        self.show_bg() || self.show_sprites()
    }

    #[inline]
    fn vram_increment(&self) -> u16 {
        if (self.ctrl & 0x04) != 0 {
            32
        } else {
            1
        }
    }

    /// Handles CPU reads from PPU registers `$2000..=$2007`.
    pub fn cpu_read(&mut self, cart: &Cartridge, reg: u16) -> u8 {
        match reg & 0x0007 {
            2 => {
                // $2002 PPUSTATUS
                let value = (self.status & 0xE0) | (self.open_bus & 0x1F);
                self.status &= !0x80; // Clear VBlank
                self.write_latch = false;
                self.open_bus = value;
                value
            }
            4 => {
                // $2004 OAMDATA
                let value = self.oam[self.oam_addr as usize];
                self.open_bus = value;
                value
            }
            7 => {
                // $2007 PPUDATA (buffered for < $3F00, immediate for palette RAM)
                let addr = self.v & 0x3FFF;
                let fetched = self.read_vram(cart, addr);
                let result = if addr < 0x3F00 {
                    let buffered = self.data_buffer;
                    self.data_buffer = fetched;
                    buffered
                } else {
                    self.data_buffer = self.read_vram(cart, addr - 0x1000);
                    fetched
                };
                self.v = self.v.wrapping_add(self.vram_increment()) & 0x7FFF;
                self.open_bus = result;
                result
            }
            _ => self.open_bus,
        }
    }

    /// Handles CPU writes to PPU registers `$2000..=$2007`.
    pub fn cpu_write(&mut self, cart: &mut Cartridge, reg: u16, data: u8) {
        self.open_bus = data;
        match reg & 0x0007 {
            0 => {
                // $2000 PPUCTRL
                let prev_nmi_enable = (self.ctrl & 0x80) != 0;
                self.ctrl = data;
                self.t = (self.t & !0x0C00) | (((data as u16) & 0x03) << 10);
                let now_nmi_enable = (self.ctrl & 0x80) != 0;
                if !prev_nmi_enable && now_nmi_enable && (self.status & 0x80) != 0 {
                    self.nmi_pending = true;
                }
            }
            1 => {
                // $2001 PPUMASK
                self.mask = data;
            }
            3 => {
                // $2003 OAMADDR
                self.oam_addr = data;
            }
            4 => {
                // $2004 OAMDATA
                self.oam[self.oam_addr as usize] = data;
                self.oam_addr = self.oam_addr.wrapping_add(1);
            }
            5 => {
                // $2005 PPUSCROLL
                if !self.write_latch {
                    self.fine_x = data & 0x07;
                    self.t = (self.t & !0x001F) | ((data as u16) >> 3);
                    self.write_latch = true;
                } else {
                    self.t = (self.t & !0x73E0)
                        | (((data as u16) & 0x07) << 12)
                        | (((data as u16) & 0xF8) << 2);
                    self.write_latch = false;
                }
            }
            6 => {
                // $2006 PPUADDR
                if !self.write_latch {
                    self.t = (self.t & 0x00FF) | (((data as u16) & 0x3F) << 8);
                    self.write_latch = true;
                } else {
                    self.t = (self.t & 0xFF00) | (data as u16);
                    self.v = self.t;
                    self.write_latch = false;
                }
            }
            7 => {
                // $2007 PPUDATA
                let addr = self.v & 0x3FFF;
                self.write_vram(cart, addr, data);
                self.v = self.v.wrapping_add(self.vram_increment()) & 0x7FFF;
            }
            _ => {}
        }
    }

    /// Advances the PPU by `cycles` dots (3 dots per CPU cycle).
    pub fn tick(&mut self, cart: &Cartridge, cycles: u32) {
        for _ in 0..cycles {
            self.step_dot(cart);
        }
    }

    #[inline]
    fn step_dot(&mut self, cart: &Cartridge) {
        if self.scanline < SCREEN_HEIGHT as u16 {
            if self.dot == 1 {
                self.render_scanline(cart, self.scanline as usize);
            }
            if let Some(hit_x) = self.pending_sprite0_hit_x {
                if self.dot == hit_x + 1 {
                    self.status |= 0x40; // Sprite 0 Hit
                    self.pending_sprite0_hit_x = None;
                }
            }
            if self.rendering_enabled() {
                if self.dot == 256 {
                    self.increment_y();
                } else if self.dot == 257 {
                    self.copy_x();
                }
            }
        } else if self.scanline == 241 {
            if self.dot == 1 {
                self.status |= 0x80; // VBlank started
                if (self.ctrl & 0x80) != 0 {
                    self.nmi_pending = true;
                }
            }
        } else if self.scanline == 261 {
            if self.dot == 1 {
                self.status &= !0xE0; // Clear VBlank, Sprite 0 Hit, Overflow
                self.pending_sprite0_hit_x = None;
            }
            if self.rendering_enabled() {
                if self.dot == 257 {
                    self.copy_x();
                } else if self.dot == 280 {
                    self.copy_y();
                }
            }
        }

        self.dot += 1;
        if self.dot >= DOTS_PER_SCANLINE {
            self.dot = 0;
            self.scanline += 1;
            if self.scanline >= SCANLINES_PER_FRAME {
                self.scanline = 0;
            }
        }
    }

    fn render_scanline(&mut self, cart: &Cartridge, y: usize) {
        let row_offset = y * SCREEN_WIDTH * 4;
        let backdrop_idx = (self.palette_ram[0] & 0x3F) as usize;
        let (br, bg, bb) = NES_PALETTE[backdrop_idx];

        // Fill scanline with universal backdrop color
        for x in 0..SCREEN_WIDTH {
            let off = row_offset + x * 4;
            self.framebuffer[off] = br;
            self.framebuffer[off + 1] = bg;
            self.framebuffer[off + 2] = bb;
            self.framebuffer[off + 3] = 255;
        }

        let mut bg_opaque = [false; SCREEN_WIDTH];

        // --- BACKGROUND RENDER PASS ---
        if self.show_bg() {
            let bg_pattern_base: u16 = if (self.ctrl & 0x10) != 0 {
                0x1000
            } else {
                0x0000
            };
            let fine_y = (self.v >> 12) & 0x07;
            let fine_x = self.fine_x as usize;
            let show_bg_left = self.show_bg_left();

            let mut cur_v = self.v;
            let mut screen_x_start: isize = -(fine_x as isize);

            // Up to 33 tiles cover 256 pixels when fine_x > 0
            for _ in 0..33 {
                let nt_addr = 0x2000 | (cur_v & 0x0FFF);
                let tile_id = self.read_vram(cart, nt_addr) as u16;

                let attr_addr = 0x23C0
                    | (cur_v & 0x0C00)
                    | ((cur_v >> 4) & 0x38)
                    | ((cur_v >> 2) & 0x07);
                let attr_byte = self.read_vram(cart, attr_addr);
                let shift = ((cur_v >> 4) & 4) | (cur_v & 2);
                let palette_id = ((attr_byte >> shift) & 0x03) as usize;

                let tile_addr = bg_pattern_base + tile_id * 16 + fine_y;
                let lo = cart.ppu_read_chr(tile_addr);
                let hi = cart.ppu_read_chr(tile_addr + 8);

                for bit_idx in 0..8isize {
                    let sx = screen_x_start + bit_idx;
                    if sx < 0 {
                        continue;
                    }
                    if sx >= SCREEN_WIDTH as isize {
                        break;
                    }
                    let sx_u = sx as usize;
                    if sx_u < 8 && !show_bg_left {
                        continue;
                    }

                    let shift_bit = 7 - bit_idx;
                    let color_lo = (lo >> shift_bit) & 1;
                    let color_hi = (hi >> shift_bit) & 1;
                    let color_idx = ((color_hi << 1) | color_lo) as usize;

                    if color_idx != 0 {
                        bg_opaque[sx_u] = true;
                        let pal_color =
                            (self.palette_ram[palette_id * 4 + color_idx] & 0x3F) as usize;
                        let (r, g, b) = NES_PALETTE[pal_color];
                        let off = row_offset + sx_u * 4;
                        self.framebuffer[off] = r;
                        self.framebuffer[off + 1] = g;
                        self.framebuffer[off + 2] = b;
                    }
                }

                cur_v = Self::increment_coarse_x(cur_v);
                screen_x_start += 8;
                if screen_x_start >= SCREEN_WIDTH as isize {
                    break;
                }
            }
        }

        // --- SPRITE (OAM) RENDER PASS ---
        self.pending_sprite0_hit_x = None;
        if self.show_sprites() {
            let tall_sprites = (self.ctrl & 0x20) != 0;
            let sprite_height: i32 = if tall_sprites { 16 } else { 8 };
            let sprite_table_base: u16 = if (self.ctrl & 0x08) != 0 {
                0x1000
            } else {
                0x0000
            };
            let show_sprites_left = self.show_sprites_left();
            let bg_enabled = self.show_bg();

            let mut sprite_drawn = [false; SCREEN_WIDTH];
            let mut sprites_on_line = 0u8;

            for i in 0..64usize {
                let base = i * 4;
                let sprite_y = self.oam[base] as i32 + 1;
                let row = y as i32 - sprite_y;
                if row < 0 || row >= sprite_height {
                    continue;
                }

                sprites_on_line += 1;
                if sprites_on_line > 8 {
                    self.status |= 0x20; // Sprite overflow
                    break;
                }

                let tile = self.oam[base + 1];
                let attr = self.oam[base + 2];
                let sprite_x = self.oam[base + 3] as usize;

                let palette_id = ((attr & 0x03) + 4) as usize;
                let behind_bg = (attr & 0x20) != 0;
                let flip_h = (attr & 0x40) != 0;
                let flip_v = (attr & 0x80) != 0;

                let effective_row = if flip_v {
                    (sprite_height - 1 - row) as u16
                } else {
                    row as u16
                };

                let tile_addr = if tall_sprites {
                    let bank = ((tile & 0x01) as u16) * 0x1000;
                    let top_tile = (tile & 0xFE) as u16;
                    let sub_tile = if effective_row >= 8 {
                        top_tile + 1
                    } else {
                        top_tile
                    };
                    bank + sub_tile * 16 + (effective_row & 7)
                } else {
                    sprite_table_base + (tile as u16) * 16 + effective_row
                };

                let lo = cart.ppu_read_chr(tile_addr);
                let hi = cart.ppu_read_chr(tile_addr + 8);

                for px in 0..8usize {
                    let sx = sprite_x + px;
                    if sx >= SCREEN_WIDTH {
                        break;
                    }
                    if sx < 8 && !show_sprites_left {
                        continue;
                    }

                    let bit = if flip_h { px } else { 7 - px };
                    let color_lo = (lo >> bit) & 1;
                    let color_hi = (hi >> bit) & 1;
                    let color_idx = ((color_hi << 1) | color_lo) as usize;

                    if color_idx == 0 {
                        continue;
                    }

                    // Sprite 0 Hit check (requires both BG & Sprites enabled, x != 255)
                    if i == 0
                        && bg_enabled
                        && bg_opaque[sx]
                        && sx < 255
                        && self.pending_sprite0_hit_x.is_none()
                    {
                        self.pending_sprite0_hit_x = Some(sx as u16);
                    }

                    if !sprite_drawn[sx] {
                        sprite_drawn[sx] = true;
                        if !behind_bg || !bg_opaque[sx] {
                            let pal_color =
                                (self.palette_ram[palette_id * 4 + color_idx] & 0x3F) as usize;
                            let (r, g, b) = NES_PALETTE[pal_color];
                            let off = row_offset + sx * 4;
                            self.framebuffer[off] = r;
                            self.framebuffer[off + 1] = g;
                            self.framebuffer[off + 2] = b;
                        }
                    }
                }
            }
        }
    }

    #[inline]
    fn increment_coarse_x(v: u16) -> u16 {
        if (v & 0x001F) == 31 {
            (v & !0x001F) ^ 0x0400
        } else {
            v + 1
        }
    }

    #[inline]
    fn increment_y(&mut self) {
        if (self.v & 0x7000) != 0x7000 {
            self.v += 0x1000;
        } else {
            self.v &= !0x7000;
            let mut y = (self.v & 0x03E0) >> 5;
            if y == 29 {
                y = 0;
                self.v ^= 0x0800;
            } else if y == 31 {
                y = 0;
            } else {
                y += 1;
            }
            self.v = (self.v & !0x03E0) | (y << 5);
        }
    }

    #[inline]
    fn copy_x(&mut self) {
        self.v = (self.v & !0x041F) | (self.t & 0x041F);
    }

    #[inline]
    fn copy_y(&mut self) {
        self.v = (self.v & !0x7BE0) | (self.t & 0x7BE0);
    }

    #[inline]
    fn mirror_nametable_addr(mirroring: Mirroring, addr: u16) -> usize {
        let offset = (addr & 0x0FFF) as usize;
        let table = offset / 0x0400;
        let within = offset % 0x0400;
        match mirroring {
            Mirroring::Horizontal => {
                let bank = if table < 2 { 0 } else { 1 };
                bank * 0x0400 + within
            }
            Mirroring::Vertical => {
                let bank = table & 1;
                bank * 0x0400 + within
            }
            Mirroring::FourScreen => offset,
        }
    }

    #[inline]
    fn mirror_palette_addr(addr: u16) -> usize {
        let idx = (addr as usize) & 0x1F;
        match idx {
            0x10 | 0x14 | 0x18 | 0x1C => idx - 0x10,
            _ => idx,
        }
    }

    fn read_vram(&self, cart: &Cartridge, addr: u16) -> u8 {
        let addr = addr & 0x3FFF;
        match addr {
            0x0000..=0x1FFF => cart.ppu_read_chr(addr),
            0x2000..=0x3EFF => {
                let idx = Self::mirror_nametable_addr(cart.mirroring(), addr - 0x2000);
                self.vram[idx]
            }
            0x3F00..=0x3FFF => self.palette_ram[Self::mirror_palette_addr(addr)],
            _ => 0,
        }
    }

    fn write_vram(&mut self, cart: &mut Cartridge, addr: u16, data: u8) {
        let addr = addr & 0x3FFF;
        match addr {
            0x0000..=0x1FFF => cart.ppu_write_chr(addr, data),
            0x2000..=0x3EFF => {
                let idx = Self::mirror_nametable_addr(cart.mirroring(), addr - 0x2000);
                self.vram[idx] = data;
            }
            0x3F00..=0x3FFF => {
                let idx = Self::mirror_palette_addr(addr);
                self.palette_ram[idx] = data;
            }
            _ => {}
        }
    }
}

impl Default for Ppu {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vblank_triggers_nmi_at_scanline_241_dot_1() {
        let mut rom = vec![0u8; 16 + 16384 + 8192];
        rom[0..4].copy_from_slice(&[0x4E, 0x45, 0x53, 0x1A]);
        rom[4] = 1;
        rom[5] = 1;
        let mut cart = Cartridge::from_bytes(&rom).unwrap();

        let mut ppu = Ppu::new();
        ppu.cpu_write(&mut cart, 0, 0x80); // Enable NMI

        // Advance to scanline 241, dot 2 (241 * 341 + 2 = 82183 dots)
        ppu.tick(&cart, 241 * 341 + 2);
        assert!(ppu.poll_nmi(), "NMI should trigger on VBlank entry");
        assert_eq!(ppu.cpu_read(&cart, 2) & 0x80, 0x80);
        // Reading $2002 clears VBlank
        assert_eq!(ppu.cpu_read(&cart, 2) & 0x80, 0x00);
    }
}
