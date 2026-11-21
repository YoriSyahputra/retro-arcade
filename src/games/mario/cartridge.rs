use std::fs;
use std::path::Path;

/// iNES magic signature: `N`, `E`, `S`, `0x1A` (MS-DOS EOF).
const INES_MAGIC: [u8; 4] = [0x4E, 0x45, 0x53, 0x1A];
const INES_HEADER_SIZE: usize = 16;
const TRAINER_SIZE: usize = 512;
const PRG_BANK_SIZE: usize = 16 * 1024;
const CHR_BANK_SIZE: usize = 8 * 1024;

/// Nametable mirroring arrangement reported in the iNES header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mirroring {
    Horizontal,
    Vertical,
    FourScreen,
}

/// Errors encountered while loading or validating an iNES ROM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CartridgeError {
    NotFound,
    IoError,
    InvalidHeader,
    UnsupportedMapper(u8),
    InvalidPrgSize,
    TruncatedRom,
}

impl CartridgeError {
    pub const fn message(self) -> &'static str {
        match self {
            CartridgeError::NotFound => "ROM file not found at assets/roms/mario.nes",
            CartridgeError::IoError => "Failed to read ROM file from disk",
            CartridgeError::InvalidHeader => "Invalid iNES header (missing NES<EOF> signature)",
            CartridgeError::UnsupportedMapper(_) => "Unsupported mapper (only Mapper 0 / NROM is supported)",
            CartridgeError::InvalidPrgSize => "Invalid PRG-ROM bank count for NROM (expected 16KB or 32KB)",
            CartridgeError::TruncatedRom => "ROM file is truncated or incomplete",
        }
    }
}

/// Parsed NES Mapper 0 (NROM-128 / NROM-256) cartridge data.
pub struct Cartridge {
    prg_rom: Vec<u8>,
    chr_mem: Vec<u8>,
    chr_is_ram: bool,
    mirroring: Mirroring,
}

impl Cartridge {
    /// Loads and parses an iNES ROM file from disk.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, CartridgeError> {
        let data = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Err(CartridgeError::NotFound);
            }
            Err(_) => return Err(CartridgeError::IoError),
        };
        Self::from_bytes(&data)
    }

    /// Parses an iNES ROM from raw bytes, supporting Mapper 0 (NROM-128 and NROM-256).
    pub fn from_bytes(raw: &[u8]) -> Result<Self, CartridgeError> {
        if raw.len() < INES_HEADER_SIZE || raw[0..4] != INES_MAGIC {
            return Err(CartridgeError::InvalidHeader);
        }

        let prg_banks = raw[4] as usize;
        let chr_banks = raw[5] as usize;
        let flags6 = raw[6];
        let flags7 = raw[7];

        let mapper = (flags7 & 0xF0) | (flags6 >> 4);
        if mapper != 0 {
            return Err(CartridgeError::UnsupportedMapper(mapper));
        }

        if prg_banks != 1 && prg_banks != 2 {
            return Err(CartridgeError::InvalidPrgSize);
        }

        let mirroring = if (flags6 & 0x08) != 0 {
            Mirroring::FourScreen
        } else if (flags6 & 0x01) != 0 {
            Mirroring::Vertical
        } else {
            Mirroring::Horizontal
        };

        let has_trainer = (flags6 & 0x04) != 0;
        let prg_offset = INES_HEADER_SIZE + if has_trainer { TRAINER_SIZE } else { 0 };
        let prg_len = prg_banks * PRG_BANK_SIZE;
        let chr_len = chr_banks * CHR_BANK_SIZE;

        if raw.len() < prg_offset + prg_len + chr_len {
            return Err(CartridgeError::TruncatedRom);
        }

        let prg_rom = raw[prg_offset..prg_offset + prg_len].to_vec();

        let (chr_mem, chr_is_ram) = if chr_banks == 0 {
            (vec![0u8; CHR_BANK_SIZE], true)
        } else {
            let chr_offset = prg_offset + prg_len;
            (raw[chr_offset..chr_offset + chr_len].to_vec(), false)
        };

        Ok(Self {
            prg_rom,
            chr_mem,
            chr_is_ram,
            mirroring,
        })
    }

    #[inline]
    pub const fn mirroring(&self) -> Mirroring {
        self.mirroring
    }

    /// Reads a byte from CPU PRG-ROM space (`$8000..=$FFFF`), mirroring 16KB NROM-128 banks.
    #[inline]
    pub fn cpu_read_prg(&self, addr: u16) -> u8 {
        let offset = (addr.wrapping_sub(0x8000)) as usize;
        let masked = if self.prg_rom.len() == PRG_BANK_SIZE {
            offset & 0x3FFF
        } else {
            offset & 0x7FFF
        };
        self.prg_rom[masked]
    }

    /// Reads a byte from PPU CHR pattern table space (`$0000..=$1FFF`).
    #[inline]
    pub fn ppu_read_chr(&self, addr: u16) -> u8 {
        self.chr_mem[(addr as usize) & 0x1FFF]
    }

    /// Writes to CHR memory if the cartridge uses CHR-RAM (no-op for CHR-ROM).
    #[inline]
    pub fn ppu_write_chr(&mut self, addr: u16, data: u8) {
        if self.chr_is_ram {
            self.chr_mem[(addr as usize) & 0x1FFF] = data;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_nrom_rom(prg_banks: u8, chr_banks: u8, flags6: u8) -> Vec<u8> {
        let mut rom = Vec::new();
        rom.extend_from_slice(&INES_MAGIC);
        rom.push(prg_banks);
        rom.push(chr_banks);
        rom.push(flags6);
        rom.push(0); // flags7 (mapper 0)
        rom.extend_from_slice(&[0u8; 8]);
        rom.resize(
            INES_HEADER_SIZE
                + (prg_banks as usize) * PRG_BANK_SIZE
                + (chr_banks as usize) * CHR_BANK_SIZE,
            0,
        );
        rom
    }

    #[test]
    fn parses_nrom128_and_mirrors_prg() {
        let mut rom = build_nrom_rom(1, 1, 0x01); // 16KB PRG, 8KB CHR, Vertical mirroring
        rom[INES_HEADER_SIZE + 0x3FFC] = 0x00;
        rom[INES_HEADER_SIZE + 0x3FFD] = 0x80;

        let cart = Cartridge::from_bytes(&rom).expect("valid NROM-128");
        assert_eq!(cart.mirroring(), Mirroring::Vertical);
        assert_eq!(cart.cpu_read_prg(0xBFFC), 0x00);
        assert_eq!(cart.cpu_read_prg(0xFFFC), 0x00);
        assert_eq!(cart.cpu_read_prg(0xFFFD), 0x80);
    }

    #[test]
    fn rejects_unsupported_mapper() {
        let mut rom = build_nrom_rom(2, 1, 0x10); // Mapper 1 (MMC1)
        rom[6] = 0x10;
        assert_eq!(
            Cartridge::from_bytes(&rom).err(),
            Some(CartridgeError::UnsupportedMapper(1))
        );
    }
}
