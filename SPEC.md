# TECHNICAL SPECIFICATION: NATIVE RETRO ARCADE HUB (RUST)

## 1. Executive Summary

A native Linux desktop application built from scratch in Rust targeting Ubuntu systems. The application serves as an integrated retro arcade hub housing multiple classic games under a single binary. It avoids web tech (Electron, Tauri, Chromium wrappers) in favor of direct hardware-accelerated 2D rendering via OpenGL/GLES.

### Key Targets

- **Target Platform:** Linux x86_64 (Ubuntu 20.04/22.04/24.04/25.04/26.05 LTS native ELF)
- **Primary Framework:** `macroquad` (v0.4)
- **Target Memory Usage:** $\le 45\text{ MB}$ RSS across all scenes
- **Target CPU Usage:** $\le 5\%$ on contemporary dual-core x86_64 CPUs
- **Binary Footprint:** Stripped release binary $\le 15\text{ MB}$

---

## 2. System Architecture & Lifecycle

### 2.1 Scene Management Architecture

The hub uses a state-driven Scene Manager pattern with dynamic dispatch or enum-based scene states. Transitions clean up old scene memory immediately.

```rust
// Contract for all game sub-modules
pub trait GameScene {
    fn update(&mut self) -> SceneAction;
    fn draw(&self);
}

pub enum SceneAction {
    None,
    SwitchTo(SceneId),
    QuitToMenu,
}

pub enum SceneId {
    Menu,
    Tetris,
    Snake,
    Mario,
}
```

### 2.2 Global Rules

1. **Universal Exit:** Pressing `Escape` in any game scene immediately returns `SceneAction::QuitToMenu`, dropping the active game state from heap memory.
2. **Virtual Canvas:** The display renders to an internal virtual canvas ($960 \times 720$), scaled with preserved aspect ratio (letterboxing/pillarboxing) to whatever window size the user sets.
3. **Zero Allocation in Loop:** No dynamic vector reallocation, string formatting, or heap clones during `update()` or `draw()`.

---

## 3. Directory Layout

```text
retro-arcade/
├── .gitignore               # Excludes build target & proprietary ROMs
├── Cargo.toml
├── SPEC.md
├── assets/
│   ├── roms/
│   │   ├── .gitkeep
│   │   └── mario.nes        # Super Mario Bros ROM (Ignored by Git)
│   ├── fonts/
│   └── sfx/
├── src/
│   ├── main.rs              # Entry point, window setup, and scene router
│   ├── core/
│   │   ├── mod.rs
│   │   ├── scene.rs         # GameScene trait & SceneAction enum
│   │   └── input.rs         # Input mapping helper
│   ├── ui/
│   │   ├── mod.rs
│   │   └── menu.rs          # Hub launcher menu with game cards
│   └── games/
│       ├── tetris/          # Pure Rust Tetris implementation
│       │   ├── mod.rs       # TetrisScene
│       │   ├── board.rs     # 10x20 matrix & line clearing
│       │   ├── piece.rs     # 7 Tetromino definitions & color palettes
│       │   └── srs.rs       # Super Rotation System & wall-kick tables
│       ├── snake/           # Pure Rust Snake with Google-like modifiers
│       │   ├── mod.rs       # SnakeScene & configuration UI
│       │   ├── entity.rs    # Snake body queue & fruit entities
│       │   └── modes.rs     # Modifiers: Portal, Speed, Twin Fruit
│       └── mario/           # Embedded Headless NES Core
│           ├── mod.rs       # MarioScene & Framebuffer blitter
│           ├── bus.rs       # System Bus (CPU/PPU memory addressing)
│           ├── cpu.rs       # Ricoh 2A03 (MOS 6502 without BCD mode)
│           ├── ppu.rs       # Picture Processing Unit (256x240 RGB)
│           ├── cartridge.rs # iNES format parser (Mapper 0 / NROM)
│           └── joypad.rs    # NES Joypad register strobe ($4016)
```

---

## 4. Module Specifications

### 4.1 Hub Menu (`src/ui/menu.rs`)

- **Visuals:** Retro arcade aesthetic, title banner, preview cards for Tetris, Snake, and Super Mario Bros.
- **Navigation:**
  - `Arrow Up` / `Arrow Down` or `Arrow Left` / `Arrow Right`: Change selected game.
  - `Enter` / `Space`: Launch the selected game.
  - `Q` or `Escape`: Quit application cleanly.

### 4.2 Game 1: Tetris (`src/games/tetris/`)

- **Playfield Matrix:** 10 visible columns $\times$ 20 visible rows, plus 2 hidden buffer rows above.
- **Pieces:** Standard 7 Tetrominoes (I, J, L, O, S, T, Z) mapped to SRS standard colors.
- **RNG:** Standard 7-Bag randomizer (prevents long droughts of any single piece).
- **SRS (Super Rotation System):**
  - Clockwise and Counter-Clockwise rotation.
  - Official SRS wall-kick offset tables for standard pieces and I-piece.
- **Mechanics:**
  - `Left` / `Right`: Shift piece horizontally.
  - `Up` / `X`: Rotate Clockwise.
  - `Z` / `Control`: Rotate Counter-Clockwise.
  - `Down`: Soft drop (accelerated downward fall).
  - `Space`: Hard drop (instant lock + score bonus).
  - `C` / `Shift`: Hold piece (maximum 1 swap per placement).
  - Ghost piece display indicating exact drop landing coordinate.
  - Line clear detection with standard scoring (Single, Double, Triple, Tetris).
  - Progressive gravity speed curve per 10 lines cleared.

### 4.3 Game 2: Snake with Modifiers (`src/games/snake/`)

- **Playfield Grid:** $20 \times 20$ grid units.
- **Snake Body:** Ring buffer or `VecDeque<(i32, i32)>` representing body coordinates.
- **Pre-Game Modifier Select Screen:**
  - **Wall Mode:**
    - `Classic`: Colliding with perimeter walls results in Game Over.
    - `Portal`: Snake wraps around screen edges seamlessly.
  - **Fruit Multiplier:**
    - `Single Apple`: 1 fruit active at all times.
    - `Double Apple`: 2 fruits active simultaneously.
  - **Special Items Toggle:**
    - `Red Apple`: Standard fruit (+1 length, +10 points).
    - `Golden Apple`: Spawns with a 7-second countdown (+3 length, +50 points).
    - `Chili Pepper`: Speeds up game tick by $1.8\times$ for 5 seconds.
- **Anti-Reversal Input Buffer:** Keypress queue that prevents instant 180-degree self-collision when pressing two directions in rapid succession.

### 4.4 Game 3: Super Mario Bros via Headless NES Core (`src/games/mario/`)

- **Cartridge Parser (`cartridge.rs`):**
  - Parses iNES header (`NES<EOF>`, 16-byte header).
  - Supports **Mapper 0 (NROM-128 and NROM-256)**:
    - PRG-ROM: 16 KB or 32 KB.
    - CHR-ROM: 8 KB pattern table.
    - Mirroring mode: Horizontal or Vertical nametable mirroring.
- **CPU Core (`cpu.rs`):**
  - Emulates Ricoh 2A03 (MOS 6502 core with BCD disabled).
  - All standard addressing modes (Immediate, ZeroPage, Absolute, Indirect, Indexed).
  - Interrupts: RESET, NMI (triggered by PPU VBlank), and IRQ.
- **PPU Core (`ppu.rs`):**
  - Emulates PPU registers `$2000` through `$2007`.
  - Scanline cycle renderer ($341 \times 262$ cycles per frame).
  - Background tile generation via nametable and pattern tables.
  - Sprite (OAM) rendering (8x8 mode, sprite priority, flip X/Y).
  - Output target: Framebuffer `[u8; 256 * 240 * 4]` (RGBA format).
- **Joypad Register (`joypad.rs`):**
  - Shift register strobe mapped to memory address `$4016`.
  - Macroquad keyboard bindings:
    - `Z` or `J` $\rightarrow$ NES B (Run / Fireball)
    - `X` or `K` $\rightarrow$ NES A (Jump)
    - `Right Shift` $\rightarrow$ NES Select
    - `Enter` $\rightarrow$ NES Start
    - `Arrow Keys` $\rightarrow$ NES D-Pad (Up, Down, Left, Right)
- **Host Integration (`mod.rs`):**
  - Checks if `assets/roms/mario.nes` exists on launch.
  - If missing, displays an informative in-game banner explaining where to place the ROM.
  - Synchronizes 29,780 CPU cycles per 60 FPS frame.
  - Updates a `macroquad::texture::Texture2D` from the raw framebuffer and blits it scaled with a 4:3 aspect ratio.

---

## 5. Dependencies (`Cargo.toml`)

```toml
[package]
name = "retro-arcade"
version = "0.1.0"
edition = "2021"

[dependencies]
macroquad = "0.4"

[profile.release]
opt-level = 3
lto = true
codegen-units = 1
panic = "abort"
strip = true
```

---

## 6. Phased Antigravity Prompts

### Phase 1: Hub Skeleton & Scene Router

```text
Role: Rust Systems and Game Engine Developer.
Task: Scaffold the "retro-arcade" project based on SPEC.md.

Directives:
1. Initialize Cargo.toml with macroquad 0.4 and release profile optimizations.
2. Implement src/core/scene.rs with the `GameScene` trait, `SceneAction`, and `SceneId` enums.
3. Implement src/ui/menu.rs as an interactive arcade selection menu with keyboard navigation.
4. Implement src/main.rs: Set 960x720 window, maintain active scene state, handle transitions, and ensure 'Escape' returns to menu.
5. Create temporary placeholder scenes for Tetris, Snake, and Mario that compile cleanly.
6. Verify compilation with `cargo check`.
```

### Phase 2: Tetris Implementation

```text
Task: Implement the complete Tetris module under src/games/tetris/ according to SPEC.md.

Directives:
1. Implement piece.rs: 7 Tetromino types, 4x4 coordinate grids, distinct colors, and a 7-bag randomizer.
2. Implement srs.rs: Official SRS rotation rules with wall-kick translation lookup tables.
3. Implement board.rs: 10x20 matrix, collision detection, line clear logic, and gravity timer.
4. Implement mod.rs: TetrisScene implementing GameScene. Include Ghost piece preview, Soft/Hard drop, Hold slot, and HUD (Score, Level, Lines, Next).
5. Verify zero runtime heap allocations during gameplay with `cargo check`.
```

### Phase 3: Snake with Google-Style Modifiers

```text
Task: Implement the Snake module under src/games/snake/ according to SPEC.md.

Directives:
1. Implement modes.rs: Struct for active game modifiers (WallMode: Classic vs Portal, Special items toggle).
2. Implement entity.rs: Snake body queue (VecDeque), fruit spawning, and movement anti-reversal buffer.
3. Implement special fruits: Red (+1), Golden Apple (timer, +3), Chili Pepper (speed boost).
4. Implement mod.rs: SnakeScene implementing GameScene. Include a pre-game modifier selection screen before starting.
5. Verify build with `cargo check`.
```

### Phase 4: Headless NES Core & Super Mario Bros

```text
Task: Implement the embedded headless NES emulator core under src/games/mario/ according to SPEC.md.

Directives:
1. Implement cartridge.rs: Parse iNES format, extract PRG-ROM (16KB/32KB) and CHR-ROM (8KB), supporting Mapper 0 (NROM).
2. Implement cpu.rs: Ricoh 2A03 (MOS 6502 without BCD mode) with all official opcodes needed for SMB.
3. Implement ppu.rs: Emulate PPU registers $2000-$2007, background tile rendering, OAM sprites, and output to a `[u8; 256 * 240 * 4]` RGBA framebuffer.
4. Implement joypad.rs: Map keyboard controls to shift register $4016.
5. Implement mod.rs: MarioScene implementing GameScene. Check for assets/roms/mario.nes (show graceful fallback if missing). Update a Macroquad Texture2D each frame and render at 4:3 ratio.
6. Verify with `cargo run`.
```

---

## 7. Version Control & Git Workflow

### 7.1 `.gitignore` Specification

Create a `.gitignore` file at the repository root immediately. **Crucial:** Never commit proprietary game ROMs (`*.nes`) to a public repository to avoid DMCA/copyright strikes.

```gitignore
# Rust build artifacts
/target/
**/*.rs.bk
Cargo.lock

# IDE & Editor configs
.vscode/
.idea/
*.swp
*.swo

# Binary Assets & Proprietary ROMs
assets/roms/*.nes
!assets/roms/.gitkeep

# Operating System specific
.DS_Store
Thumbs.db
```

### 7.2 Initial Repository Setup

Run these commands in Ubuntu terminal inside the project root:

```bash
# Inisialisasi Git repository
git init -b main

# Buat placeholder agar folder assets/roms tetap terdeteksi oleh Git
mkdir -p assets/roms assets/fonts assets/sfx
touch assets/roms/.gitkeep

# Stage file awal (SPEC.md, Cargo.toml, .gitignore)
git add .gitignore SPEC.md Cargo.toml assets/roms/.gitkeep

# Commit perdana
git commit -m "chore: initial project blueprint and specifications"

# Hubungkan ke remote GitHub / GitLab (ganti URL dengan repository kamu)
git remote add origin git@github.com:USERNAME/retro-arcade.git
# atau jika menggunakan HTTPS:
# git remote add origin https://github.com/USERNAME/retro-arcade.git

# Push cabang utama
git push -u origin main
```

### 7.3 Phased Commit Strategy (Conventional Commits)

Lakukan commit terpisah setiap kali satu fase Antigravity berhasil dikompilasi (`cargo check` atau `cargo run` sukses):

#### Setelah Fase 1 Selesai:

```bash
git add src/core/ src/ui/ src/main.rs Cargo.toml
git commit -m "feat(core): scaffold scene manager and launcher menu"
git push
```

#### Setelah Fase 2 Selesai:

```bash
git add src/games/tetris/
git commit -m "feat(tetris): implement full SRS rotation, 7-bag, and board mechanics"
git push
```

#### Setelah Fase 3 Selesai:

```bash
git add src/games/snake/
git commit -m "feat(snake): add Google-style modifiers, special fruits, and portal mode"
git push
```

#### Setelah Fase 4 Selesai:

```bash
git add src/games/mario/
git commit -m "feat(mario): implement headless NES emulator core and NROM mapper"
git push
```

### 7.4 Release Tagging

Ketika semua game sudah stabil dan siap dijalankan:

```bash
git tag -a v1.0.0 -m "Release v1.0.0: Native Linux Retro Arcade Hub"
git push origin v1.0.0
```
