use crate::games::mario::cartridge::Cartridge;
use crate::games::mario::joypad::Joypad;
use crate::games::mario::ppu::{Ppu, FRAMEBUFFER_SIZE};

/// NES System Bus interconnecting 2KB CPU RAM, 2C02 PPU, Joypad `$4016`, and NROM Cartridge.
pub struct Bus {
    cpu_ram: [u8; 2048],
    pub ppu: Ppu,
    pub cartridge: Cartridge,
    pub joypad1: Joypad,
    dma_stall_cycles: u16,
    irq_pending: bool,
}

impl Bus {
    pub fn new(cartridge: Cartridge) -> Self {
        Self {
            cpu_ram: [0; 2048],
            ppu: Ppu::new(),
            cartridge,
            joypad1: Joypad::new(),
            dma_stall_cycles: 0,
            irq_pending: false,
        }
    }

    pub fn reset(&mut self) {
        self.ppu.reset();
        self.dma_stall_cycles = 0;
        self.irq_pending = false;
    }

    #[inline]
    pub fn framebuffer(&self) -> &[u8; FRAMEBUFFER_SIZE] {
        self.ppu.framebuffer()
    }

    #[inline]
    pub fn set_joypad_buttons(&mut self, mask: u8) {
        self.joypad1.set_buttons(mask);
    }

    /// Consumes any pending OAM DMA stall cycles triggered by writes to `$4014`.
    #[inline]
    pub fn take_dma_stall_cycles(&mut self) -> u16 {
        let cycles = self.dma_stall_cycles;
        self.dma_stall_cycles = 0;
        cycles
    }

    #[inline]
    pub fn poll_nmi(&mut self) -> bool {
        self.ppu.poll_nmi()
    }

    #[inline]
    pub fn poll_irq(&mut self) -> bool {
        let pending = self.irq_pending;
        self.irq_pending = false;
        pending
    }

    /// Advances the PPU by `cpu_cycles * 3` PPU dots.
    #[inline]
    pub fn tick_ppu_for_cpu_cycles(&mut self, cpu_cycles: u32) {
        self.ppu.tick(&self.cartridge, cpu_cycles * 3);
    }

    /// CPU memory space read (`$0000..=$FFFF`).
    pub fn cpu_read(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x1FFF => self.cpu_ram[(addr & 0x07FF) as usize],
            0x2000..=0x3FFF => self.ppu.cpu_read(&self.cartridge, addr & 0x0007),
            0x4016 => self.joypad1.read(),
            0x4017 => 0x40, // Controller 2 disconnected open-bus
            0x4000..=0x401F => 0x00, // Headless APU / I/O registers
            0x8000..=0xFFFF => self.cartridge.cpu_read_prg(addr),
            _ => 0x00,
        }
    }

    /// CPU memory space write (`$0000..=$FFFF`).
    pub fn cpu_write(&mut self, addr: u16, data: u8) {
        match addr {
            0x0000..=0x1FFF => {
                self.cpu_ram[(addr & 0x07FF) as usize] = data;
            }
            0x2000..=0x3FFF => {
                self.ppu.cpu_write(&mut self.cartridge, addr & 0x0007, data);
            }
            0x4014 => {
                // OAM DMA: transfer 256 bytes from CPU page `data << 8` into PPU OAM
                let base = (data as u16) << 8;
                for offset in 0..256u16 {
                    let byte = self.cpu_read(base | offset);
                    self.ppu.write_oam_dma_byte(byte);
                }
                self.dma_stall_cycles = self.dma_stall_cycles.saturating_add(513);
            }
            0x4016 => {
                self.joypad1.write(data);
            }
            0x4000..=0x401F => {
                // Headless APU registers ignored
            }
            _ => {}
        }
    }

    /// Convenience little-endian 16-bit read.
    #[inline]
    pub fn cpu_read_u16(&mut self, addr: u16) -> u16 {
        let lo = self.cpu_read(addr) as u16;
        let hi = self.cpu_read(addr.wrapping_add(1)) as u16;
        (hi << 8) | lo
    }
}
