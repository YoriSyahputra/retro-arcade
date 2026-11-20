use macroquad::prelude::{is_key_pressed, KeyCode};

/// Returns true if the universal exit key (`Escape`) was pressed this frame.
#[inline]
pub fn is_escape_pressed() -> bool {
    is_key_pressed(KeyCode::Escape)
}

/// Returns true if the user navigated to the previous menu item.
#[inline]
pub fn is_menu_prev_pressed() -> bool {
    is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::Left)
}

/// Returns true if the user navigated to the next menu item.
#[inline]
pub fn is_menu_next_pressed() -> bool {
    is_key_pressed(KeyCode::Down) || is_key_pressed(KeyCode::Right)
}

/// Returns true if the user confirmed a menu selection.
#[inline]
pub fn is_menu_confirm_pressed() -> bool {
    is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Space)
}

/// Returns true if the user pressed the quit shortcut (`Q`) in the menu.
#[inline]
pub fn is_menu_quit_pressed() -> bool {
    is_key_pressed(KeyCode::Q)
}
