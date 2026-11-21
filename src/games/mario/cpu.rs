use crate::games::mario::bus::Bus;

/// Processor status flag bits (`P` register).
pub mod flag {
    pub const CARRY: u8 = 1 << 0;
    pub const ZERO: u8 = 1 << 1;
    pub const INTERRUPT_DISABLE: u8 = 1 << 2;
    pub const DECIMAL: u8 = 1 << 3;
    pub const BREAK: u8 = 1 << 4;
    pub const UNUSED: u8 = 1 << 5;
    pub const OVERFLOW: u8 = 1 << 6;
    pub const NEGATIVE: u8 = 1 << 7;
}

/// Emulates the Ricoh 2A03 CPU (MOS 6502 core with hardware BCD arithmetic disabled).
pub struct Cpu {
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub sp: u8,
    pub pc: u16,
    pub status: u8,
    pub total_cycles: u64,
}

impl Cpu {
    pub const fn new() -> Self {
        Self {
            a: 0,
            x: 0,
            y: 0,
            sp: 0xFD,
            pc: 0x8000,
            status: flag::INTERRUPT_DISABLE | flag::UNUSED,
            total_cycles: 0,
        }
    }

    /// Executes the hardware RESET sequence, loading `PC` from vector `$FFFC/$FFFD`.
    pub fn reset(&mut self, bus: &mut Bus) {
        self.a = 0;
        self.x = 0;
        self.y = 0;
        self.sp = 0xFD;
        self.status = flag::INTERRUPT_DISABLE | flag::UNUSED;
        self.pc = bus.cpu_read_u16(0xFFFC);
        self.total_cycles = 7;
    }

    /// Services a Non-Maskable Interrupt (VBlank NMI from PPU), vector `$FFFA/$FFFB`.
    pub fn nmi(&mut self, bus: &mut Bus) -> u32 {
        self.push_u16(bus, self.pc);
        let pushed_status = (self.status & !flag::BREAK) | flag::UNUSED;
        self.push_u8(bus, pushed_status);
        self.set_flag(flag::INTERRUPT_DISABLE, true);
        self.pc = bus.cpu_read_u16(0xFFFA);
        self.total_cycles += 7;
        7
    }

    /// Services a maskable IRQ if the `I` flag is clear, vector `$FFFE/$FFFF`.
    pub fn irq(&mut self, bus: &mut Bus) -> u32 {
        if self.get_flag(flag::INTERRUPT_DISABLE) {
            return 0;
        }
        self.push_u16(bus, self.pc);
        let pushed_status = (self.status & !flag::BREAK) | flag::UNUSED;
        self.push_u8(bus, pushed_status);
        self.set_flag(flag::INTERRUPT_DISABLE, true);
        self.pc = bus.cpu_read_u16(0xFFFE);
        self.total_cycles += 7;
        7
    }

    /// Executes one CPU instruction (or pending NMI/IRQ) and returns elapsed CPU cycles.
    pub fn step(&mut self, bus: &mut Bus) -> u32 {
        if bus.poll_nmi() {
            let cycles = self.nmi(bus);
            bus.tick_ppu_for_cpu_cycles(cycles);
            return cycles;
        }
        if bus.poll_irq() {
            let cycles = self.irq(bus);
            if cycles > 0 {
                bus.tick_ppu_for_cpu_cycles(cycles);
                return cycles;
            }
        }

        let opcode = self.fetch_u8(bus);
        let mut cycles = self.execute_opcode(bus, opcode);

        // Account for any OAM DMA stall cycles incurred during this instruction
        let dma_cycles = bus.take_dma_stall_cycles() as u32;
        if dma_cycles > 0 {
            let odd_penalty = if (self.total_cycles + cycles as u64) & 1 == 1 {
                1
            } else {
                0
            };
            cycles += dma_cycles + odd_penalty;
        }

        self.total_cycles += cycles as u64;
        bus.tick_ppu_for_cpu_cycles(cycles);
        cycles
    }

    #[inline]
    fn get_flag(&self, mask: u8) -> bool {
        (self.status & mask) != 0
    }

    #[inline]
    fn set_flag(&mut self, mask: u8, value: bool) {
        if value {
            self.status |= mask;
        } else {
            self.status &= !mask;
        }
    }

    #[inline]
    fn update_zn(&mut self, value: u8) {
        self.set_flag(flag::ZERO, value == 0);
        self.set_flag(flag::NEGATIVE, (value & 0x80) != 0);
    }

    #[inline]
    fn fetch_u8(&mut self, bus: &mut Bus) -> u8 {
        let val = bus.cpu_read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        val
    }

    #[inline]
    fn fetch_u16(&mut self, bus: &mut Bus) -> u16 {
        let lo = self.fetch_u8(bus) as u16;
        let hi = self.fetch_u8(bus) as u16;
        (hi << 8) | lo
    }

    #[inline]
    fn push_u8(&mut self, bus: &mut Bus, value: u8) {
        bus.cpu_write(0x0100 | self.sp as u16, value);
        self.sp = self.sp.wrapping_sub(1);
    }

    #[inline]
    fn pop_u8(&mut self, bus: &mut Bus) -> u8 {
        self.sp = self.sp.wrapping_add(1);
        bus.cpu_read(0x0100 | self.sp as u16)
    }

    #[inline]
    fn push_u16(&mut self, bus: &mut Bus, value: u16) {
        self.push_u8(bus, (value >> 8) as u8);
        self.push_u8(bus, (value & 0xFF) as u8);
    }

    #[inline]
    fn pop_u16(&mut self, bus: &mut Bus) -> u16 {
        let lo = self.pop_u8(bus) as u16;
        let hi = self.pop_u8(bus) as u16;
        (hi << 8) | lo
    }

    // --- ADDRESSING MODE HELPERS ---

    #[inline]
    fn addr_zp(&mut self, bus: &mut Bus) -> u16 {
        self.fetch_u8(bus) as u16
    }

    #[inline]
    fn addr_zpx(&mut self, bus: &mut Bus) -> u16 {
        self.fetch_u8(bus).wrapping_add(self.x) as u16
    }

    #[inline]
    fn addr_zpy(&mut self, bus: &mut Bus) -> u16 {
        self.fetch_u8(bus).wrapping_add(self.y) as u16
    }

    #[inline]
    fn addr_abs(&mut self, bus: &mut Bus) -> u16 {
        self.fetch_u16(bus)
    }

    #[inline]
    fn addr_absx(&mut self, bus: &mut Bus) -> (u16, bool) {
        let base = self.fetch_u16(bus);
        let addr = base.wrapping_add(self.x as u16);
        (addr, (base & 0xFF00) != (addr & 0xFF00))
    }

    #[inline]
    fn addr_absy(&mut self, bus: &mut Bus) -> (u16, bool) {
        let base = self.fetch_u16(bus);
        let addr = base.wrapping_add(self.y as u16);
        (addr, (base & 0xFF00) != (addr & 0xFF00))
    }

    #[inline]
    fn addr_indx(&mut self, bus: &mut Bus) -> u16 {
        let ptr = self.fetch_u8(bus).wrapping_add(self.x);
        let lo = bus.cpu_read(ptr as u16) as u16;
        let hi = bus.cpu_read(ptr.wrapping_add(1) as u16) as u16;
        (hi << 8) | lo
    }

    #[inline]
    fn addr_indy(&mut self, bus: &mut Bus) -> (u16, bool) {
        let zp = self.fetch_u8(bus);
        let lo = bus.cpu_read(zp as u16) as u16;
        let hi = bus.cpu_read(zp.wrapping_add(1) as u16) as u16;
        let base = (hi << 8) | lo;
        let addr = base.wrapping_add(self.y as u16);
        (addr, (base & 0xFF00) != (addr & 0xFF00))
    }

    // --- ALU & CONTROL HELPERS ---

    #[inline]
    fn adc(&mut self, operand: u8) {
        let carry = u16::from(self.get_flag(flag::CARRY));
        let sum = self.a as u16 + operand as u16 + carry;
        let result = sum as u8;
        let overflow = (!(self.a ^ operand) & (self.a ^ result) & 0x80) != 0;
        self.set_flag(flag::CARRY, sum > 0xFF);
        self.set_flag(flag::OVERFLOW, overflow);
        self.a = result;
        self.update_zn(self.a);
    }

    #[inline]
    fn sbc(&mut self, operand: u8) {
        // Ricoh 2A03 disables BCD mode: binary SBC is identical to ADC(!operand).
        self.adc(!operand);
    }

    #[inline]
    fn compare(&mut self, reg: u8, operand: u8) {
        let diff = reg.wrapping_sub(operand);
        self.set_flag(flag::CARRY, reg >= operand);
        self.update_zn(diff);
    }

    #[inline]
    fn bit_test(&mut self, operand: u8) {
        self.set_flag(flag::ZERO, (self.a & operand) == 0);
        self.set_flag(flag::OVERFLOW, (operand & 0x40) != 0);
        self.set_flag(flag::NEGATIVE, (operand & 0x80) != 0);
    }

    #[inline]
    fn branch(&mut self, bus: &mut Bus, condition: bool) -> u32 {
        let offset = self.fetch_u8(bus) as i8;
        if !condition {
            return 2;
        }
        let old_pc = self.pc;
        self.pc = self.pc.wrapping_add(offset as i16 as u16);
        if (old_pc & 0xFF00) != (self.pc & 0xFF00) {
            4
        } else {
            3
        }
    }

    #[inline]
    fn asl_val(&mut self, val: u8) -> u8 {
        self.set_flag(flag::CARRY, (val & 0x80) != 0);
        let res = val << 1;
        self.update_zn(res);
        res
    }

    #[inline]
    fn lsr_val(&mut self, val: u8) -> u8 {
        self.set_flag(flag::CARRY, (val & 0x01) != 0);
        let res = val >> 1;
        self.update_zn(res);
        res
    }

    #[inline]
    fn rol_val(&mut self, val: u8) -> u8 {
        let old_c = u8::from(self.get_flag(flag::CARRY));
        self.set_flag(flag::CARRY, (val & 0x80) != 0);
        let res = (val << 1) | old_c;
        self.update_zn(res);
        res
    }

    #[inline]
    fn ror_val(&mut self, val: u8) -> u8 {
        let old_c = u8::from(self.get_flag(flag::CARRY)) << 7;
        self.set_flag(flag::CARRY, (val & 0x01) != 0);
        let res = (val >> 1) | old_c;
        self.update_zn(res);
        res
    }

    fn execute_opcode(&mut self, bus: &mut Bus, opcode: u8) -> u32 {
        match opcode {
            // --- LDA ---
            0xA9 => { let v = self.fetch_u8(bus); self.a = v; self.update_zn(v); 2 }
            0xA5 => { let a = self.addr_zp(bus); let v = bus.cpu_read(a); self.a = v; self.update_zn(v); 3 }
            0xB5 => { let a = self.addr_zpx(bus); let v = bus.cpu_read(a); self.a = v; self.update_zn(v); 4 }
            0xAD => { let a = self.addr_abs(bus); let v = bus.cpu_read(a); self.a = v; self.update_zn(v); 4 }
            0xBD => { let (a, c) = self.addr_absx(bus); let v = bus.cpu_read(a); self.a = v; self.update_zn(v); 4 + u32::from(c) }
            0xB9 => { let (a, c) = self.addr_absy(bus); let v = bus.cpu_read(a); self.a = v; self.update_zn(v); 4 + u32::from(c) }
            0xA1 => { let a = self.addr_indx(bus); let v = bus.cpu_read(a); self.a = v; self.update_zn(v); 6 }
            0xB1 => { let (a, c) = self.addr_indy(bus); let v = bus.cpu_read(a); self.a = v; self.update_zn(v); 5 + u32::from(c) }

            // --- LDX ---
            0xA2 => { let v = self.fetch_u8(bus); self.x = v; self.update_zn(v); 2 }
            0xA6 => { let a = self.addr_zp(bus); let v = bus.cpu_read(a); self.x = v; self.update_zn(v); 3 }
            0xB6 => { let a = self.addr_zpy(bus); let v = bus.cpu_read(a); self.x = v; self.update_zn(v); 4 }
            0xAE => { let a = self.addr_abs(bus); let v = bus.cpu_read(a); self.x = v; self.update_zn(v); 4 }
            0xBE => { let (a, c) = self.addr_absy(bus); let v = bus.cpu_read(a); self.x = v; self.update_zn(v); 4 + u32::from(c) }

            // --- LDY ---
            0xA0 => { let v = self.fetch_u8(bus); self.y = v; self.update_zn(v); 2 }
            0xA4 => { let a = self.addr_zp(bus); let v = bus.cpu_read(a); self.y = v; self.update_zn(v); 3 }
            0xB4 => { let a = self.addr_zpx(bus); let v = bus.cpu_read(a); self.y = v; self.update_zn(v); 4 }
            0xAC => { let a = self.addr_abs(bus); let v = bus.cpu_read(a); self.y = v; self.update_zn(v); 4 }
            0xBC => { let (a, c) = self.addr_absx(bus); let v = bus.cpu_read(a); self.y = v; self.update_zn(v); 4 + u32::from(c) }

            // --- STA ---
            0x85 => { let a = self.addr_zp(bus); bus.cpu_write(a, self.a); 3 }
            0x95 => { let a = self.addr_zpx(bus); bus.cpu_write(a, self.a); 4 }
            0x8D => { let a = self.addr_abs(bus); bus.cpu_write(a, self.a); 4 }
            0x9D => { let (a, _) = self.addr_absx(bus); bus.cpu_write(a, self.a); 5 }
            0x99 => { let (a, _) = self.addr_absy(bus); bus.cpu_write(a, self.a); 5 }
            0x81 => { let a = self.addr_indx(bus); bus.cpu_write(a, self.a); 6 }
            0x91 => { let (a, _) = self.addr_indy(bus); bus.cpu_write(a, self.a); 6 }

            // --- STX ---
            0x86 => { let a = self.addr_zp(bus); bus.cpu_write(a, self.x); 3 }
            0x96 => { let a = self.addr_zpy(bus); bus.cpu_write(a, self.x); 4 }
            0x8E => { let a = self.addr_abs(bus); bus.cpu_write(a, self.x); 4 }

            // --- STY ---
            0x84 => { let a = self.addr_zp(bus); bus.cpu_write(a, self.y); 3 }
            0x94 => { let a = self.addr_zpx(bus); bus.cpu_write(a, self.y); 4 }
            0x8C => { let a = self.addr_abs(bus); bus.cpu_write(a, self.y); 4 }

            // --- Register Transfers ---
            0xAA => { self.x = self.a; self.update_zn(self.x); 2 } // TAX
            0xA8 => { self.y = self.a; self.update_zn(self.y); 2 } // TAY
            0xBA => { self.x = self.sp; self.update_zn(self.x); 2 } // TSX
            0x8A => { self.a = self.x; self.update_zn(self.a); 2 } // TXA
            0x9A => { self.sp = self.x; 2 } // TXS
            0x98 => { self.a = self.y; self.update_zn(self.a); 2 } // TYA

            // --- Stack ---
            0x48 => { self.push_u8(bus, self.a); 3 } // PHA
            0x08 => { self.push_u8(bus, self.status | flag::BREAK | flag::UNUSED); 3 } // PHP
            0x68 => { self.a = self.pop_u8(bus); self.update_zn(self.a); 4 } // PLA
            0x28 => { self.status = (self.pop_u8(bus) & !flag::BREAK) | flag::UNUSED; 4 } // PLP

            // --- Logical AND ---
            0x29 => { let v = self.fetch_u8(bus); self.a &= v; self.update_zn(self.a); 2 }
            0x25 => { let a = self.addr_zp(bus); self.a &= bus.cpu_read(a); self.update_zn(self.a); 3 }
            0x35 => { let a = self.addr_zpx(bus); self.a &= bus.cpu_read(a); self.update_zn(self.a); 4 }
            0x2D => { let a = self.addr_abs(bus); self.a &= bus.cpu_read(a); self.update_zn(self.a); 4 }
            0x3D => { let (a, c) = self.addr_absx(bus); self.a &= bus.cpu_read(a); self.update_zn(self.a); 4 + u32::from(c) }
            0x39 => { let (a, c) = self.addr_absy(bus); self.a &= bus.cpu_read(a); self.update_zn(self.a); 4 + u32::from(c) }
            0x21 => { let a = self.addr_indx(bus); self.a &= bus.cpu_read(a); self.update_zn(self.a); 6 }
            0x31 => { let (a, c) = self.addr_indy(bus); self.a &= bus.cpu_read(a); self.update_zn(self.a); 5 + u32::from(c) }

            // --- Logical EOR ---
            0x49 => { let v = self.fetch_u8(bus); self.a ^= v; self.update_zn(self.a); 2 }
            0x45 => { let a = self.addr_zp(bus); self.a ^= bus.cpu_read(a); self.update_zn(self.a); 3 }
            0x55 => { let a = self.addr_zpx(bus); self.a ^= bus.cpu_read(a); self.update_zn(self.a); 4 }
            0x4D => { let a = self.addr_abs(bus); self.a ^= bus.cpu_read(a); self.update_zn(self.a); 4 }
            0x5D => { let (a, c) = self.addr_absx(bus); self.a ^= bus.cpu_read(a); self.update_zn(self.a); 4 + u32::from(c) }
            0x59 => { let (a, c) = self.addr_absy(bus); self.a ^= bus.cpu_read(a); self.update_zn(self.a); 4 + u32::from(c) }
            0x41 => { let a = self.addr_indx(bus); self.a ^= bus.cpu_read(a); self.update_zn(self.a); 6 }
            0x51 => { let (a, c) = self.addr_indy(bus); self.a ^= bus.cpu_read(a); self.update_zn(self.a); 5 + u32::from(c) }

            // --- Logical ORA ---
            0x09 => { let v = self.fetch_u8(bus); self.a |= v; self.update_zn(self.a); 2 }
            0x05 => { let a = self.addr_zp(bus); self.a |= bus.cpu_read(a); self.update_zn(self.a); 3 }
            0x15 => { let a = self.addr_zpx(bus); self.a |= bus.cpu_read(a); self.update_zn(self.a); 4 }
            0x0D => { let a = self.addr_abs(bus); self.a |= bus.cpu_read(a); self.update_zn(self.a); 4 }
            0x1D => { let (a, c) = self.addr_absx(bus); self.a |= bus.cpu_read(a); self.update_zn(self.a); 4 + u32::from(c) }
            0x19 => { let (a, c) = self.addr_absy(bus); self.a |= bus.cpu_read(a); self.update_zn(self.a); 4 + u32::from(c) }
            0x01 => { let a = self.addr_indx(bus); self.a |= bus.cpu_read(a); self.update_zn(self.a); 6 }
            0x11 => { let (a, c) = self.addr_indy(bus); self.a |= bus.cpu_read(a); self.update_zn(self.a); 5 + u32::from(c) }

            // --- BIT ---
            0x24 => { let a = self.addr_zp(bus); let v = bus.cpu_read(a); self.bit_test(v); 3 }
            0x2C => { let a = self.addr_abs(bus); let v = bus.cpu_read(a); self.bit_test(v); 4 }

            // --- ADC ---
            0x69 => { let v = self.fetch_u8(bus); self.adc(v); 2 }
            0x65 => { let a = self.addr_zp(bus); let v = bus.cpu_read(a); self.adc(v); 3 }
            0x75 => { let a = self.addr_zpx(bus); let v = bus.cpu_read(a); self.adc(v); 4 }
            0x6D => { let a = self.addr_abs(bus); let v = bus.cpu_read(a); self.adc(v); 4 }
            0x7D => { let (a, c) = self.addr_absx(bus); let v = bus.cpu_read(a); self.adc(v); 4 + u32::from(c) }
            0x79 => { let (a, c) = self.addr_absy(bus); let v = bus.cpu_read(a); self.adc(v); 4 + u32::from(c) }
            0x61 => { let a = self.addr_indx(bus); let v = bus.cpu_read(a); self.adc(v); 6 }
            0x71 => { let (a, c) = self.addr_indy(bus); let v = bus.cpu_read(a); self.adc(v); 5 + u32::from(c) }

            // --- SBC ---
            0xE9 => { let v = self.fetch_u8(bus); self.sbc(v); 2 }
            0xE5 => { let a = self.addr_zp(bus); let v = bus.cpu_read(a); self.sbc(v); 3 }
            0xF5 => { let a = self.addr_zpx(bus); let v = bus.cpu_read(a); self.sbc(v); 4 }
            0xED => { let a = self.addr_abs(bus); let v = bus.cpu_read(a); self.sbc(v); 4 }
            0xFD => { let (a, c) = self.addr_absx(bus); let v = bus.cpu_read(a); self.sbc(v); 4 + u32::from(c) }
            0xF9 => { let (a, c) = self.addr_absy(bus); let v = bus.cpu_read(a); self.sbc(v); 4 + u32::from(c) }
            0xE1 => { let a = self.addr_indx(bus); let v = bus.cpu_read(a); self.sbc(v); 6 }
            0xF1 => { let (a, c) = self.addr_indy(bus); let v = bus.cpu_read(a); self.sbc(v); 5 + u32::from(c) }

            // --- CMP ---
            0xC9 => { let v = self.fetch_u8(bus); self.compare(self.a, v); 2 }
            0xC5 => { let a = self.addr_zp(bus); let v = bus.cpu_read(a); self.compare(self.a, v); 3 }
            0xD5 => { let a = self.addr_zpx(bus); let v = bus.cpu_read(a); self.compare(self.a, v); 4 }
            0xCD => { let a = self.addr_abs(bus); let v = bus.cpu_read(a); self.compare(self.a, v); 4 }
            0xDD => { let (a, c) = self.addr_absx(bus); let v = bus.cpu_read(a); self.compare(self.a, v); 4 + u32::from(c) }
            0xD9 => { let (a, c) = self.addr_absy(bus); let v = bus.cpu_read(a); self.compare(self.a, v); 4 + u32::from(c) }
            0xC1 => { let a = self.addr_indx(bus); let v = bus.cpu_read(a); self.compare(self.a, v); 6 }
            0xD1 => { let (a, c) = self.addr_indy(bus); let v = bus.cpu_read(a); self.compare(self.a, v); 5 + u32::from(c) }

            // --- CPX ---
            0xE0 => { let v = self.fetch_u8(bus); self.compare(self.x, v); 2 }
            0xE4 => { let a = self.addr_zp(bus); let v = bus.cpu_read(a); self.compare(self.x, v); 3 }
            0xEC => { let a = self.addr_abs(bus); let v = bus.cpu_read(a); self.compare(self.x, v); 4 }

            // --- CPY ---
            0xC0 => { let v = self.fetch_u8(bus); self.compare(self.y, v); 2 }
            0xC4 => { let a = self.addr_zp(bus); let v = bus.cpu_read(a); self.compare(self.y, v); 3 }
            0xCC => { let a = self.addr_abs(bus); let v = bus.cpu_read(a); self.compare(self.y, v); 4 }

            // --- INC / DEC ---
            0xE6 => { let a = self.addr_zp(bus); let v = bus.cpu_read(a).wrapping_add(1); bus.cpu_write(a, v); self.update_zn(v); 5 }
            0xF6 => { let a = self.addr_zpx(bus); let v = bus.cpu_read(a).wrapping_add(1); bus.cpu_write(a, v); self.update_zn(v); 6 }
            0xEE => { let a = self.addr_abs(bus); let v = bus.cpu_read(a).wrapping_add(1); bus.cpu_write(a, v); self.update_zn(v); 6 }
            0xFE => { let (a, _) = self.addr_absx(bus); let v = bus.cpu_read(a).wrapping_add(1); bus.cpu_write(a, v); self.update_zn(v); 7 }

            0xC6 => { let a = self.addr_zp(bus); let v = bus.cpu_read(a).wrapping_sub(1); bus.cpu_write(a, v); self.update_zn(v); 5 }
            0xD6 => { let a = self.addr_zpx(bus); let v = bus.cpu_read(a).wrapping_sub(1); bus.cpu_write(a, v); self.update_zn(v); 6 }
            0xCE => { let a = self.addr_abs(bus); let v = bus.cpu_read(a).wrapping_sub(1); bus.cpu_write(a, v); self.update_zn(v); 6 }
            0xDE => { let (a, _) = self.addr_absx(bus); let v = bus.cpu_read(a).wrapping_sub(1); bus.cpu_write(a, v); self.update_zn(v); 7 }

            0xE8 => { self.x = self.x.wrapping_add(1); self.update_zn(self.x); 2 } // INX
            0xC8 => { self.y = self.y.wrapping_add(1); self.update_zn(self.y); 2 } // INY
            0xCA => { self.x = self.x.wrapping_sub(1); self.update_zn(self.x); 2 } // DEX
            0x88 => { self.y = self.y.wrapping_sub(1); self.update_zn(self.y); 2 } // DEY

            // --- Shifts & Rotates ---
            0x0A => { self.a = self.asl_val(self.a); 2 }
            0x06 => { let a = self.addr_zp(bus); let v = bus.cpu_read(a); let r = self.asl_val(v); bus.cpu_write(a, r); 5 }
            0x16 => { let a = self.addr_zpx(bus); let v = bus.cpu_read(a); let r = self.asl_val(v); bus.cpu_write(a, r); 6 }
            0x0E => { let a = self.addr_abs(bus); let v = bus.cpu_read(a); let r = self.asl_val(v); bus.cpu_write(a, r); 6 }
            0x1E => { let (a, _) = self.addr_absx(bus); let v = bus.cpu_read(a); let r = self.asl_val(v); bus.cpu_write(a, r); 7 }

            0x4A => { self.a = self.lsr_val(self.a); 2 }
            0x46 => { let a = self.addr_zp(bus); let v = bus.cpu_read(a); let r = self.lsr_val(v); bus.cpu_write(a, r); 5 }
            0x56 => { let a = self.addr_zpx(bus); let v = bus.cpu_read(a); let r = self.lsr_val(v); bus.cpu_write(a, r); 6 }
            0x4E => { let a = self.addr_abs(bus); let v = bus.cpu_read(a); let r = self.lsr_val(v); bus.cpu_write(a, r); 6 }
            0x5E => { let (a, _) = self.addr_absx(bus); let v = bus.cpu_read(a); let r = self.lsr_val(v); bus.cpu_write(a, r); 7 }

            0x2A => { self.a = self.rol_val(self.a); 2 }
            0x26 => { let a = self.addr_zp(bus); let v = bus.cpu_read(a); let r = self.rol_val(v); bus.cpu_write(a, r); 5 }
            0x36 => { let a = self.addr_zpx(bus); let v = bus.cpu_read(a); let r = self.rol_val(v); bus.cpu_write(a, r); 6 }
            0x2E => { let a = self.addr_abs(bus); let v = bus.cpu_read(a); let r = self.rol_val(v); bus.cpu_write(a, r); 6 }
            0x3E => { let (a, _) = self.addr_absx(bus); let v = bus.cpu_read(a); let r = self.rol_val(v); bus.cpu_write(a, r); 7 }

            0x6A => { self.a = self.ror_val(self.a); 2 }
            0x66 => { let a = self.addr_zp(bus); let v = bus.cpu_read(a); let r = self.ror_val(v); bus.cpu_write(a, r); 5 }
            0x76 => { let a = self.addr_zpx(bus); let v = bus.cpu_read(a); let r = self.ror_val(v); bus.cpu_write(a, r); 6 }
            0x6E => { let a = self.addr_abs(bus); let v = bus.cpu_read(a); let r = self.ror_val(v); bus.cpu_write(a, r); 6 }
            0x7E => { let (a, _) = self.addr_absx(bus); let v = bus.cpu_read(a); let r = self.ror_val(v); bus.cpu_write(a, r); 7 }

            // --- Jumps & Subroutines ---
            0x4C => { self.pc = self.fetch_u16(bus); 3 } // JMP Absolute
            0x6C => {
                // JMP Indirect (with authentic 6502 page-boundary wrap bug)
                let ptr = self.fetch_u16(bus);
                let lo = bus.cpu_read(ptr) as u16;
                let hi_addr = (ptr & 0xFF00) | (ptr.wrapping_add(1) & 0x00FF);
                let hi = bus.cpu_read(hi_addr) as u16;
                self.pc = (hi << 8) | lo;
                5
            }
            0x20 => {
                // JSR
                let target = self.fetch_u16(bus);
                let ret = self.pc.wrapping_sub(1);
                self.push_u16(bus, ret);
                self.pc = target;
                6
            }
            0x60 => {
                // RTS
                self.pc = self.pop_u16(bus).wrapping_add(1);
                6
            }
            0x40 => {
                // RTI
                self.status = (self.pop_u8(bus) & !flag::BREAK) | flag::UNUSED;
                self.pc = self.pop_u16(bus);
                6
            }

            // --- Branches ---
            0x90 => self.branch(bus, !self.get_flag(flag::CARRY)),    // BCC
            0xB0 => self.branch(bus, self.get_flag(flag::CARRY)),     // BCS
            0xF0 => self.branch(bus, self.get_flag(flag::ZERO)),      // BEQ
            0xD0 => self.branch(bus, !self.get_flag(flag::ZERO)),     // BNE
            0x30 => self.branch(bus, self.get_flag(flag::NEGATIVE)),  // BMI
            0x10 => self.branch(bus, !self.get_flag(flag::NEGATIVE)), // BPL
            0x50 => self.branch(bus, !self.get_flag(flag::OVERFLOW)), // BVC
            0x70 => self.branch(bus, self.get_flag(flag::OVERFLOW)),  // BVS

            // --- Status Flag Ops ---
            0x18 => { self.set_flag(flag::CARRY, false); 2 }             // CLC
            0xD8 => { self.set_flag(flag::DECIMAL, false); 2 }           // CLD
            0x58 => { self.set_flag(flag::INTERRUPT_DISABLE, false); 2 } // CLI
            0xB8 => { self.set_flag(flag::OVERFLOW, false); 2 }          // CLV
            0x38 => { self.set_flag(flag::CARRY, true); 2 }              // SEC
            0xF8 => { self.set_flag(flag::DECIMAL, true); 2 }            // SED
            0x78 => { self.set_flag(flag::INTERRUPT_DISABLE, true); 2 }  // SEI

            // --- System ---
            0x00 => {
                // BRK
                self.pc = self.pc.wrapping_add(1);
                self.push_u16(bus, self.pc);
                self.push_u8(bus, self.status | flag::BREAK | flag::UNUSED);
                self.set_flag(flag::INTERRUPT_DISABLE, true);
                self.pc = bus.cpu_read_u16(0xFFFE);
                7
            }
            0xEA => 2, // NOP

            // Safe fallback for unofficial opcodes (treated as 2-cycle NOP)
            _ => 2,
        }
    }
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::mario::cartridge::Cartridge;

    fn test_bus_with_prg(program: &[u8]) -> Bus {
        let mut rom = vec![0u8; 16 + 16384 + 8192];
        rom[0..4].copy_from_slice(&[0x4E, 0x45, 0x53, 0x1A]);
        rom[4] = 1; // 16KB NROM-128
        rom[5] = 1; // 8KB CHR
        rom[16..16 + program.len()].copy_from_slice(program);
        // Reset vector -> $8000
        rom[16 + 0x3FFC] = 0x00;
        rom[16 + 0x3FFD] = 0x80;
        let cart = Cartridge::from_bytes(&rom).unwrap();
        Bus::new(cart)
    }

    #[test]
    fn adc_ignores_bcd_mode_like_ricoh_2a03() {
        // SED; LDA #$09; CLC; ADC #$01 -> in binary mode = $0A (not $10!)
        let prog = [0xF8, 0xA9, 0x09, 0x18, 0x69, 0x01];
        let mut bus = test_bus_with_prg(&prog);
        let mut cpu = Cpu::new();
        cpu.reset(&mut bus);

        for _ in 0..4 {
            cpu.step(&mut bus);
        }
        assert!(cpu.get_flag(flag::DECIMAL));
        assert_eq!(cpu.a, 0x0A);
    }

    #[test]
    fn jsr_rts_and_stack_operations_work() {
        // $8000: JSR $8006; INX; NOP; ($8006:) LDA #$42; RTS
        let prog = [0x20, 0x06, 0x80, 0xE8, 0xEA, 0xEA, 0xA9, 0x42, 0x60];
        let mut bus = test_bus_with_prg(&prog);
        let mut cpu = Cpu::new();
        cpu.reset(&mut bus);

        cpu.step(&mut bus); // JSR -> $8006
        assert_eq!(cpu.pc, 0x8006);
        cpu.step(&mut bus); // LDA #$42
        cpu.step(&mut bus); // RTS -> $8003
        assert_eq!(cpu.pc, 0x8003);
        cpu.step(&mut bus); // INX
        assert_eq!(cpu.a, 0x42);
        assert_eq!(cpu.x, 1);
    }

    #[test]
    fn jmp_indirect_emulates_6502_page_boundary_bug() {
        // Write pointer bytes at $02FF = $34 and $0200 = $81, then JMP ($02FF) -> $8134
        let prog = [0x6C, 0xFF, 0x02];
        let mut bus = test_bus_with_prg(&prog);
        bus.cpu_write(0x02FF, 0x34);
        bus.cpu_write(0x0200, 0x81);
        bus.cpu_write(0x0300, 0x99); // Must NOT be read due to 6502 wrap bug

        let mut cpu = Cpu::new();
        cpu.reset(&mut bus);
        cpu.step(&mut bus);
        assert_eq!(cpu.pc, 0x8134);
    }
}
