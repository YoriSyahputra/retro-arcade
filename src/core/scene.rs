/// Identifies each scene available in the retro arcade hub.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneId {
    Menu,
    Tetris,
    Snake,
    Mario,
}

/// Actions returned by a scene update tick to instruct the scene router.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneAction {
    None,
    SwitchTo(SceneId),
    QuitToMenu,
    QuitApp,
}

/// Contract for all scenes (hub menu and playable game sub-modules).
pub trait GameScene {
    fn update(&mut self) -> SceneAction;
    fn draw(&self);
}
