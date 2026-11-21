/// NES standard controller button bitmask constants (serial read order).
pub mod button {
    pub const A: u8 = 1 << 0;
    pub const B: u8 = 1 << 1;
    pub const SELECT: u8 = 1 << 2;
    pub const START: u8 = 1 << 3;
    pub const UP: u8 = 1 << 4;
    pub const DOWN: u8 = 1 << 5;
    pub const LEFT: u8 = 1 << 6;
    pub const RIGHT: u8 = 1 << 7;
}

/// Emulates the NES controller shift register mapped at CPU address `$4016`.
#[derive(Debug, Default, Clone, Copy)]
pub struct Joypad {
    strobe: bool,
    button_index: u8,
    button_status: u8,
}

impl Joypad {
    pub const fn new() -> Self {
        Self {
            strobe: false,
            button_index: 0,
            button_status: 0,
        }
    }

    /// Updates the live host button bitmask (`button::A`, `button::B`, etc.).
    #[inline]
    pub fn set_buttons(&mut self, mask: u8) {
        self.button_status = mask;
    }

    /// Handles CPU writes to `$4016` (bit 0 controls shift-register strobe).
    #[inline]
    pub fn write(&mut self, data: u8) {
        self.strobe = (data & 0x01) != 0;
        if self.strobe {
            self.button_index = 0;
        }
    }

    /// Handles CPU reads from `$4016`, shifting out one button bit per read.
    /// Returns `0x40 | bit` (matching open-bus upper bits on NES hardware).
    #[inline]
    pub fn read(&mut self) -> u8 {
        if self.button_index >= 8 {
            return 0x41;
        }
        let bit = (self.button_status >> self.button_index) & 0x01;
        if !self.strobe {
            self.button_index += 1;
        }
        0x40 | bit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joypad_strobe_and_serial_shift_order() {
        let mut pad = Joypad::new();
        pad.set_buttons(button::A | button::START | button::RIGHT);

        // Strobe high -> low latches buttons
        pad.write(1);
        assert_eq!(pad.read() & 1, 1); // Always A while strobe=1
        assert_eq!(pad.read() & 1, 1);
        pad.write(0);

        let expected = [1, 0, 0, 1, 0, 0, 0, 1]; // A, B, Sel, Start, Up, Down, Left, Right
        for bit in expected {
            assert_eq!(pad.read() & 1, bit);
        }
        // Subsequent reads after 8 bits return 1
        assert_eq!(pad.read() & 1, 1);
    }
}
