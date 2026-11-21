/// Perimeter wall behavior for the Snake playfield.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WallMode {
    /// Colliding with the arena perimeter ends the run.
    Classic,
    /// Crossing an edge wraps seamlessly to the opposite side.
    Portal,
}

impl WallMode {
    pub const fn toggle(self) -> Self {
        match self {
            WallMode::Classic => WallMode::Portal,
            WallMode::Portal => WallMode::Classic,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            WallMode::Classic => "CLASSIC",
            WallMode::Portal => "PORTAL",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            WallMode::Classic => "Walls are solid; hitting the border ends the game.",
            WallMode::Portal => "Edges wrap around seamlessly to the opposite side.",
        }
    }
}

/// Number of standard Red Apples maintained simultaneously on the grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FruitMultiplier {
    Single,
    Double,
}

impl FruitMultiplier {
    pub const fn count(self) -> usize {
        match self {
            FruitMultiplier::Single => 1,
            FruitMultiplier::Double => 2,
        }
    }

    pub const fn toggle(self) -> Self {
        match self {
            FruitMultiplier::Single => FruitMultiplier::Double,
            FruitMultiplier::Double => FruitMultiplier::Single,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            FruitMultiplier::Single => "SINGLE APPLE",
            FruitMultiplier::Double => "DOUBLE APPLE",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            FruitMultiplier::Single => "1 standard red apple active at a time.",
            FruitMultiplier::Double => "2 standard red apples active simultaneously.",
        }
    }
}

/// Active modifier configuration chosen on the pre-game selection screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnakeModifiers {
    pub wall_mode: WallMode,
    pub fruit_multiplier: FruitMultiplier,
    pub special_items: bool,
}

impl SnakeModifiers {
    pub const fn new() -> Self {
        Self {
            wall_mode: WallMode::Classic,
            fruit_multiplier: FruitMultiplier::Single,
            special_items: true,
        }
    }

    pub const fn special_label(self) -> &'static str {
        if self.special_items {
            "ENABLED"
        } else {
            "DISABLED"
        }
    }
}

impl Default for SnakeModifiers {
    fn default() -> Self {
        Self::new()
    }
}
