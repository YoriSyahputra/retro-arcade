use std::collections::VecDeque;

use crate::games::snake::modes::{SnakeModifiers, WallMode};

/// Grid dimensions (20x20 cells).
pub const GRID_SIZE: i32 = 20;
/// Total number of grid cells (`20 * 20 = 400`).
pub const MAX_CELLS: usize = (GRID_SIZE * GRID_SIZE) as usize;

/// Red apple rewards.
pub const RED_APPLE_POINTS: u32 = 10;
pub const RED_APPLE_GROWTH: u32 = 1;

/// Golden apple rewards and board countdown.
pub const GOLDEN_APPLE_POINTS: u32 = 50;
pub const GOLDEN_APPLE_GROWTH: u32 = 3;
pub const GOLDEN_APPLE_DURATION: f32 = 7.0;

/// Chili pepper rewards, board lifetime, and speed boost parameters.
pub const CHILI_PEPPER_POINTS: u32 = 15;
pub const CHILI_BOARD_DURATION: f32 = 9.0;
pub const CHILI_BOOST_DURATION: f32 = 5.0;
pub const CHILI_SPEED_MULTIPLIER: f32 = 1.8;

/// Cardinal movement directions on the 20x20 grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Direction {
    pub const fn delta(self) -> (i32, i32) {
        match self {
            Direction::Up => (0, -1),
            Direction::Down => (0, 1),
            Direction::Left => (-1, 0),
            Direction::Right => (1, 0),
        }
    }

    pub const fn is_opposite(self, other: Self) -> bool {
        matches!(
            (self, other),
            (Direction::Up, Direction::Down)
                | (Direction::Down, Direction::Up)
                | (Direction::Left, Direction::Right)
                | (Direction::Right, Direction::Left)
        )
    }
}

/// Fixed-capacity anti-reversal keypress queue (zero heap allocations).
/// Prevents instant 180-degree self-collision when two directions are tapped in rapid succession.
#[derive(Debug, Clone, Copy)]
pub struct DirectionQueue {
    buf: [Direction; 3],
    len: usize,
}

impl DirectionQueue {
    pub const fn new() -> Self {
        Self {
            buf: [Direction::Right; 3],
            len: 0,
        }
    }

    pub fn clear(&mut self) {
        self.len = 0;
    }

    /// Enqueues `next` if it is neither identical nor opposite to the last planned direction.
    pub fn push(&mut self, current_dir: Direction, next: Direction) {
        if self.len >= self.buf.len() {
            return;
        }
        let reference = if self.len == 0 {
            current_dir
        } else {
            self.buf[self.len - 1]
        };
        if next == reference || next.is_opposite(reference) {
            return;
        }
        self.buf[self.len] = next;
        self.len += 1;
    }

    /// Pops the oldest queued direction, if any.
    pub fn pop(&mut self) -> Option<Direction> {
        if self.len == 0 {
            return None;
        }
        let first = self.buf[0];
        for i in 1..self.len {
            self.buf[i - 1] = self.buf[i];
        }
        self.len -= 1;
        Some(first)
    }
}

impl Default for DirectionQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of advancing the snake by one grid tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepOutcome {
    Moved((i32, i32)),
    Collided,
}

/// Snake entity backed by a pre-allocated `VecDeque<(i32, i32)>` so runtime moves never reallocate.
pub struct Snake {
    body: VecDeque<(i32, i32)>,
    direction: Direction,
    input_queue: DirectionQueue,
    pending_growth: u32,
}

impl Snake {
    /// Constructs a snake with capacity pre-reserved for the entire 20x20 arena.
    pub fn new() -> Self {
        let mut snake = Self {
            body: VecDeque::with_capacity(MAX_CELLS),
            direction: Direction::Right,
            input_queue: DirectionQueue::new(),
            pending_growth: 0,
        };
        snake.reset();
        snake
    }

    /// Resets the snake to a 3-segment starting posture near the center of the arena.
    pub fn reset(&mut self) {
        self.body.clear();
        self.body.push_back((10, 10)); // Head
        self.body.push_back((9, 10));
        self.body.push_back((8, 10));  // Tail
        self.direction = Direction::Right;
        self.input_queue.clear();
        self.pending_growth = 0;
    }

    #[inline]
    pub fn body(&self) -> &VecDeque<(i32, i32)> {
        &self.body
    }

    #[inline]
    pub fn head(&self) -> (i32, i32) {
        self.body.front().copied().unwrap_or((10, 10))
    }

    #[inline]
    pub fn direction(&self) -> Direction {
        self.direction
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.body.len()
    }

    #[inline]
    pub fn is_full(&self) -> bool {
        self.body.len() >= MAX_CELLS
    }

    pub fn occupies(&self, pos: (i32, i32)) -> bool {
        self.body.iter().any(|&seg| seg == pos)
    }

    pub fn queue_direction(&mut self, dir: Direction) {
        self.input_queue.push(self.direction, dir);
    }

    pub fn add_growth(&mut self, amount: u32) {
        self.pending_growth = self.pending_growth.saturating_add(amount);
    }

    /// Advances the snake by one grid cell, enforcing `wall_mode` and self-collision rules.
    pub fn step(&mut self, wall_mode: WallMode) -> StepOutcome {
        if let Some(next_dir) = self.input_queue.pop() {
            if !next_dir.is_opposite(self.direction) {
                self.direction = next_dir;
            }
        }

        let (hx, hy) = self.head();
        let (dx, dy) = self.direction.delta();
        let mut nx = hx + dx;
        let mut ny = hy + dy;

        match wall_mode {
            WallMode::Classic => {
                if !(0..GRID_SIZE).contains(&nx) || !(0..GRID_SIZE).contains(&ny) {
                    return StepOutcome::Collided;
                }
            }
            WallMode::Portal => {
                nx = nx.rem_euclid(GRID_SIZE);
                ny = ny.rem_euclid(GRID_SIZE);
            }
        }

        let next_head = (nx, ny);
        let will_pop_tail = self.pending_growth == 0;
        let check_len = if will_pop_tail {
            self.body.len().saturating_sub(1)
        } else {
            self.body.len()
        };

        if self.body.iter().take(check_len).any(|&seg| seg == next_head) {
            return StepOutcome::Collided;
        }

        if will_pop_tail {
            self.body.pop_back();
        } else {
            self.pending_growth -= 1;
        }
        self.body.push_front(next_head);

        StepOutcome::Moved(next_head)
    }
}

impl Default for Snake {
    fn default() -> Self {
        Self::new()
    }
}

/// Special timed items spawned when the Special Items modifier is enabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialKind {
    GoldenApple,
    ChiliPepper,
}

/// An active timed special item on the playfield.
#[derive(Debug, Clone, Copy)]
pub struct SpecialFruit {
    pub pos: (i32, i32),
    pub kind: SpecialKind,
    pub remaining: f32,
    pub max_duration: f32,
}

/// Effect returned when the snake's head lands on a fruit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EatOutcome {
    None,
    RedApple,
    GoldenApple,
    ChiliPepper,
}

/// Zero-allocation manager for standard Red Apples and timed Special Items.
pub struct FruitField {
    standard: [Option<(i32, i32)>; 2],
    special: Option<SpecialFruit>,
    special_cooldown: f32,
    rng_state: u64,
}

impl FruitField {
    pub fn new(seed: u64) -> Self {
        Self {
            standard: [None; 2],
            special: None,
            special_cooldown: 5.0,
            rng_state: if seed == 0 {
                0xD1B5_4A32_D192_ED03
            } else {
                seed
            },
        }
    }

    pub fn reset(&mut self, snake: &Snake, modifiers: SnakeModifiers, seed: u64) {
        self.standard = [None; 2];
        self.special = None;
        self.special_cooldown = 4.5;
        if seed != 0 {
            self.rng_state = seed;
        }
        self.refill_standard(snake, modifiers);
    }

    #[inline]
    pub fn standard_fruits(&self) -> &[Option<(i32, i32)>; 2] {
        &self.standard
    }

    #[inline]
    pub fn special_fruit(&self) -> Option<SpecialFruit> {
        self.special
    }

    /// Updates special item countdowns and spawns a Golden Apple or Chili Pepper when ready.
    pub fn update_timers(&mut self, dt: f32, snake: &Snake, modifiers: SnakeModifiers) {
        if !modifiers.special_items {
            self.special = None;
            return;
        }

        if let Some(mut item) = self.special {
            item.remaining -= dt;
            if item.remaining <= 0.0 {
                self.special = None;
                self.special_cooldown = self.next_cooldown();
            } else {
                self.special = Some(item);
            }
        } else {
            self.special_cooldown -= dt;
            if self.special_cooldown <= 0.0 {
                if let Some(pos) = self.pick_free_cell(snake) {
                    let kind = if (self.next_u64() & 1) == 0 {
                        SpecialKind::GoldenApple
                    } else {
                        SpecialKind::ChiliPepper
                    };
                    let duration = match kind {
                        SpecialKind::GoldenApple => GOLDEN_APPLE_DURATION,
                        SpecialKind::ChiliPepper => CHILI_BOARD_DURATION,
                    };
                    self.special = Some(SpecialFruit {
                        pos,
                        kind,
                        remaining: duration,
                        max_duration: duration,
                    });
                } else {
                    self.special_cooldown = 2.0;
                }
            }
        }
    }

    /// Checks whether `head` consumed any active fruit and refills standard apples immediately.
    pub fn check_eat(
        &mut self,
        head: (i32, i32),
        snake: &Snake,
        modifiers: SnakeModifiers,
    ) -> EatOutcome {
        for slot in &mut self.standard {
            if *slot == Some(head) {
                *slot = None;
                self.refill_standard(snake, modifiers);
                return EatOutcome::RedApple;
            }
        }

        if let Some(item) = self.special {
            if item.pos == head {
                self.special = None;
                self.special_cooldown = self.next_cooldown();
                return match item.kind {
                    SpecialKind::GoldenApple => EatOutcome::GoldenApple,
                    SpecialKind::ChiliPepper => EatOutcome::ChiliPepper,
                };
            }
        }

        EatOutcome::None
    }

    pub fn refill_standard(&mut self, snake: &Snake, modifiers: SnakeModifiers) {
        let target = modifiers.fruit_multiplier.count();
        for i in 0..self.standard.len() {
            if i < target {
                if self.standard[i].is_none() {
                    self.standard[i] = self.pick_free_cell(snake);
                }
            } else {
                self.standard[i] = None;
            }
        }
    }

    fn is_occupied_by_fruit(&self, pos: (i32, i32)) -> bool {
        if self.standard.contains(&Some(pos)) {
            return true;
        }
        if let Some(item) = self.special {
            if item.pos == pos {
                return true;
            }
        }
        false
    }

    /// Uniformly picks an unoccupied cell using a stack-allocated array (zero heap allocations).
    fn pick_free_cell(&mut self, snake: &Snake) -> Option<(i32, i32)> {
        let mut free = [(0i32, 0i32); MAX_CELLS];
        let mut count = 0usize;

        for y in 0..GRID_SIZE {
            for x in 0..GRID_SIZE {
                let pos = (x, y);
                if !snake.occupies(pos) && !self.is_occupied_by_fruit(pos) {
                    free[count] = pos;
                    count += 1;
                }
            }
        }

        if count == 0 {
            return None;
        }

        let idx = (self.next_u64() as usize) % count;
        Some(free[idx])
    }

    fn next_cooldown(&mut self) -> f32 {
        // Pseudo-random cooldown between 5.0 and 8.5 seconds.
        5.0 + ((self.next_u64() % 8) as f32) * 0.5
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.rng_state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng_state = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::snake::modes::FruitMultiplier;

    #[test]
    fn anti_reversal_queue_blocks_180_and_allows_rapid_u_turn() {
        let mut snake = Snake::new(); // Starts facing Right at (10,10)
        // Direct reversal Right -> Left must be ignored
        snake.queue_direction(Direction::Left);
        assert_eq!(snake.step(WallMode::Classic), StepOutcome::Moved((11, 10)));
        assert_eq!(snake.direction(), Direction::Right);

        // Rapid combo Down -> Left within one tick executes across two ticks safely
        snake.queue_direction(Direction::Down);
        snake.queue_direction(Direction::Left);
        assert_eq!(snake.step(WallMode::Classic), StepOutcome::Moved((11, 11)));
        assert_eq!(snake.direction(), Direction::Down);
        assert_eq!(snake.step(WallMode::Classic), StepOutcome::Moved((10, 11)));
        assert_eq!(snake.direction(), Direction::Left);
    }

    #[test]
    fn wall_modes_classic_collides_and_portal_wraps() {
        let mut snake = Snake::new();
        // Advance from x=10 to x=19 (9 steps)
        for _ in 0..9 {
            assert!(matches!(snake.step(WallMode::Classic), StepOutcome::Moved(_)));
        }
        assert_eq!(snake.head(), (19, 10));

        // Classic collides on x=20
        assert_eq!(snake.step(WallMode::Classic), StepOutcome::Collided);

        // Portal wraps x=19 -> x=0
        assert_eq!(snake.step(WallMode::Portal), StepOutcome::Moved((0, 10)));
    }

    #[test]
    fn double_apple_and_special_countdown_work() {
        let snake = Snake::new();
        let mods = SnakeModifiers {
            wall_mode: WallMode::Classic,
            fruit_multiplier: FruitMultiplier::Double,
            special_items: true,
        };
        let mut field = FruitField::new(99);
        field.reset(&snake, mods, 99);

        assert!(field.standard_fruits()[0].is_some());
        assert!(field.standard_fruits()[1].is_some());
        assert_ne!(field.standard_fruits()[0], field.standard_fruits()[1]);

        // Advance timers until a special fruit spawns
        field.update_timers(5.0, &snake, mods);
        let spawned = field.special_fruit().expect("special fruit should spawn");
        assert!(spawned.remaining > 0.0);

        // Expire it completely
        field.update_timers(spawned.max_duration + 0.5, &snake, mods);
        assert!(field.special_fruit().is_none());
    }
}
