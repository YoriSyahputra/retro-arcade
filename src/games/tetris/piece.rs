use macroquad::prelude::Color;

/// Standard 7 Tetromino types in SRS order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PieceKind {
    I,
    J,
    L,
    O,
    S,
    T,
    Z,
}

impl PieceKind {
    pub const ALL: [PieceKind; 7] = [
        PieceKind::I,
        PieceKind::J,
        PieceKind::L,
        PieceKind::O,
        PieceKind::S,
        PieceKind::T,
        PieceKind::Z,
    ];

    /// Official SRS color palette for each Tetromino.
    pub const fn color(self) -> Color {
        match self {
            PieceKind::I => Color::new(0.0, 0.88, 0.96, 1.0),   // Cyan
            PieceKind::J => Color::new(0.16, 0.36, 0.96, 1.0),  // Blue
            PieceKind::L => Color::new(0.98, 0.56, 0.12, 1.0),  // Orange
            PieceKind::O => Color::new(0.98, 0.86, 0.14, 1.0),  // Yellow
            PieceKind::S => Color::new(0.22, 0.88, 0.32, 1.0),  // Green
            PieceKind::T => Color::new(0.68, 0.24, 0.94, 1.0),  // Purple
            PieceKind::Z => Color::new(0.95, 0.20, 0.24, 1.0),  // Red
        }
    }

    /// Returns the 4 local `(x, y)` coordinates within the 4x4 bounding box
    /// for the given rotation state (`0` = spawn, `1` = CW, `2` = 180, `3` = CCW).
    /// Y increases downward to match playfield row indexing.
    pub const fn cells(self, rotation: u8) -> [(i32, i32); 4] {
        let rot = (rotation & 3) as usize;
        match self {
            PieceKind::I => I_SHAPES[rot],
            PieceKind::J => J_SHAPES[rot],
            PieceKind::L => L_SHAPES[rot],
            PieceKind::O => O_SHAPES[rot],
            PieceKind::S => S_SHAPES[rot],
            PieceKind::T => T_SHAPES[rot],
            PieceKind::Z => Z_SHAPES[rot],
        }
    }
}

const I_SHAPES: [[(i32, i32); 4]; 4] = [
    [(0, 1), (1, 1), (2, 1), (3, 1)],
    [(2, 0), (2, 1), (2, 2), (2, 3)],
    [(0, 2), (1, 2), (2, 2), (3, 2)],
    [(1, 0), (1, 1), (1, 2), (1, 3)],
];

const O_SHAPES: [[(i32, i32); 4]; 4] = [
    [(1, 0), (2, 0), (1, 1), (2, 1)],
    [(1, 0), (2, 0), (1, 1), (2, 1)],
    [(1, 0), (2, 0), (1, 1), (2, 1)],
    [(1, 0), (2, 0), (1, 1), (2, 1)],
];

const T_SHAPES: [[(i32, i32); 4]; 4] = [
    [(1, 0), (0, 1), (1, 1), (2, 1)],
    [(1, 0), (1, 1), (2, 1), (1, 2)],
    [(0, 1), (1, 1), (2, 1), (1, 2)],
    [(1, 0), (0, 1), (1, 1), (1, 2)],
];

const S_SHAPES: [[(i32, i32); 4]; 4] = [
    [(1, 0), (2, 0), (0, 1), (1, 1)],
    [(1, 0), (1, 1), (2, 1), (2, 2)],
    [(1, 1), (2, 1), (0, 2), (1, 2)],
    [(0, 0), (0, 1), (1, 1), (1, 2)],
];

const Z_SHAPES: [[(i32, i32); 4]; 4] = [
    [(0, 0), (1, 0), (1, 1), (2, 1)],
    [(2, 0), (1, 1), (2, 1), (1, 2)],
    [(0, 1), (1, 1), (1, 2), (2, 2)],
    [(1, 0), (0, 1), (1, 1), (0, 2)],
];

const J_SHAPES: [[(i32, i32); 4]; 4] = [
    [(0, 0), (0, 1), (1, 1), (2, 1)],
    [(1, 0), (2, 0), (1, 1), (1, 2)],
    [(0, 1), (1, 1), (2, 1), (2, 2)],
    [(1, 0), (1, 1), (0, 2), (1, 2)],
];

const L_SHAPES: [[(i32, i32); 4]; 4] = [
    [(2, 0), (0, 1), (1, 1), (2, 1)],
    [(1, 0), (1, 1), (1, 2), (2, 2)],
    [(0, 1), (1, 1), (2, 1), (0, 2)],
    [(0, 0), (1, 0), (1, 1), (1, 2)],
];

/// Represents the currently falling Tetromino on the board.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActivePiece {
    pub kind: PieceKind,
    pub x: i32,
    pub y: i32,
    pub rotation: u8,
}

impl ActivePiece {
    /// Creates a piece at the standard SRS horizontal spawn column (`x = 3`).
    pub const fn spawn(kind: PieceKind, y: i32) -> Self {
        Self {
            kind,
            x: 3,
            y,
            rotation: 0,
        }
    }

    /// Returns the 4 board-space `(col, row)` coordinates occupied by this piece.
    pub fn world_cells(&self) -> [(i32, i32); 4] {
        let local = self.kind.cells(self.rotation);
        [
            (self.x + local[0].0, self.y + local[0].1),
            (self.x + local[1].0, self.y + local[1].1),
            (self.x + local[2].0, self.y + local[2].1),
            (self.x + local[3].0, self.y + local[3].1),
        ]
    }
}

/// Zero-allocation double-buffered 7-Bag randomizer supporting lookahead preview.
pub struct SevenBag {
    current: [PieceKind; 7],
    next: [PieceKind; 7],
    index: usize,
    rng_state: u64,
}

impl SevenBag {
    /// Creates a new 7-bag randomizer with a non-zero seed.
    pub fn new(seed: u64) -> Self {
        let mut bag = Self {
            current: PieceKind::ALL,
            next: PieceKind::ALL,
            index: 0,
            rng_state: if seed == 0 {
                0x9E37_79B9_7F4A_7C15
            } else {
                seed
            },
        };
        bag.shuffle_current();
        bag.shuffle_next();
        bag
    }

    /// Pops the next Tetromino from the bag, refilling seamlessly with zero heap allocations.
    pub fn next_piece(&mut self) -> PieceKind {
        let piece = self.current[self.index];
        self.index += 1;
        if self.index >= 7 {
            self.current = self.next;
            self.index = 0;
            self.shuffle_next();
        }
        piece
    }

    /// Peeks `offset` pieces ahead (`0..=6`) without mutating the bag.
    pub fn peek(&self, offset: usize) -> PieceKind {
        let pos = self.index + offset;
        if pos < 7 {
            self.current[pos]
        } else {
            self.next[(pos - 7) % 7]
        }
    }

    fn next_u64(&mut self) -> u64 {
        // xorshift64* PRNG
        let mut x = self.rng_state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng_state = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn shuffle_current(&mut self) {
        self.current = PieceKind::ALL;
        for i in (1..7).rev() {
            let j = (self.next_u64() as usize) % (i + 1);
            self.current.swap(i, j);
        }
    }

    fn shuffle_next(&mut self) {
        self.next = PieceKind::ALL;
        for i in (1..7).rev() {
            let j = (self.next_u64() as usize) % (i + 1);
            self.next.swap(i, j);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seven_bag_contains_each_piece_once_per_bag() {
        let mut bag = SevenBag::new(42);
        for _ in 0..10 {
            let mut seen = [false; 7];
            for _ in 0..7 {
                let kind = bag.next_piece();
                let idx = PieceKind::ALL.iter().position(|&k| k == kind).unwrap();
                assert!(!seen[idx], "duplicate piece in 7-bag");
                seen[idx] = true;
            }
            assert!(seen.iter().all(|&v| v));
        }
    }

    #[test]
    fn seven_bag_peek_matches_next_piece() {
        let mut bag = SevenBag::new(1337);
        let preview = [bag.peek(0), bag.peek(1), bag.peek(2), bag.peek(3)];
        for expected in preview {
            assert_eq!(bag.next_piece(), expected);
        }
    }
}
