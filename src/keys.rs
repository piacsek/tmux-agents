use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::tmux::Key;

pub fn to_tmux(event: KeyEvent) -> Option<Key> {
    match event.code {
        KeyCode::Char(c) if event.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(Key::Named(format!("C-{c}")))
        }
        KeyCode::Char(c) if event.modifiers.contains(KeyModifiers::ALT) => {
            Some(Key::Named(format!("M-{c}")))
        }
        KeyCode::Char(c) => Some(Key::Literal(c.to_string())),
        code => named(code).map(|name| Key::Named(name.to_string())),
    }
}

fn named(code: KeyCode) -> Option<&'static str> {
    Some(match code {
        KeyCode::Enter => "Enter",
        KeyCode::Esc => "Escape",
        KeyCode::Backspace => "BSpace",
        KeyCode::Tab => "Tab",
        KeyCode::BackTab => "BTab",
        KeyCode::Up => "Up",
        KeyCode::Down => "Down",
        KeyCode::Left => "Left",
        KeyCode::Right => "Right",
        KeyCode::Home => "Home",
        KeyCode::End => "End",
        KeyCode::PageUp => "PPage",
        KeyCode::PageDown => "NPage",
        KeyCode::Delete => "DC",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(code: KeyCode) -> Option<Key> {
        to_tmux(KeyEvent::new(code, KeyModifiers::NONE))
    }

    #[test]
    fn a_character_is_typed_literally() {
        assert_eq!(
            plain(KeyCode::Char('h')),
            Some(Key::Literal("h".to_string()))
        );
    }

    #[test]
    fn editing_and_navigation_keys_use_tmux_names() {
        assert_eq!(plain(KeyCode::Enter), Some(Key::Named("Enter".to_string())));
        assert_eq!(plain(KeyCode::Esc), Some(Key::Named("Escape".to_string())));
        assert_eq!(
            plain(KeyCode::Backspace),
            Some(Key::Named("BSpace".to_string()))
        );
        assert_eq!(plain(KeyCode::Tab), Some(Key::Named("Tab".to_string())));
        assert_eq!(
            plain(KeyCode::BackTab),
            Some(Key::Named("BTab".to_string()))
        );
        assert_eq!(plain(KeyCode::Up), Some(Key::Named("Up".to_string())));
        assert_eq!(plain(KeyCode::Down), Some(Key::Named("Down".to_string())));
        assert_eq!(plain(KeyCode::Left), Some(Key::Named("Left".to_string())));
        assert_eq!(plain(KeyCode::Right), Some(Key::Named("Right".to_string())));
        assert_eq!(plain(KeyCode::Home), Some(Key::Named("Home".to_string())));
        assert_eq!(plain(KeyCode::End), Some(Key::Named("End".to_string())));
        assert_eq!(
            plain(KeyCode::PageUp),
            Some(Key::Named("PPage".to_string()))
        );
        assert_eq!(
            plain(KeyCode::PageDown),
            Some(Key::Named("NPage".to_string()))
        );
        assert_eq!(plain(KeyCode::Delete), Some(Key::Named("DC".to_string())));
    }

    #[test]
    fn control_and_alt_chords_use_tmux_prefixes() {
        let ctrl_c = to_tmux(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        let alt_b = to_tmux(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::ALT));
        assert_eq!(ctrl_c, Some(Key::Named("C-c".to_string())));
        assert_eq!(alt_b, Some(Key::Named("M-b".to_string())));
    }

    #[test]
    fn keys_tmux_cannot_name_are_dropped() {
        assert_eq!(plain(KeyCode::F(5)), None);
    }
}
