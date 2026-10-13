use std::str::FromStr;
use tauri_plugin_global_shortcut::Shortcut;

// New settings should store KeyboardEvent.code, not the Option-produced character.
// These aliases accept old ASCII single-key settings in the same physical-code
// convention used by global-hotkey. Non-ASCII layouts should use the capture UI.
pub(super) fn parse_character(character: char) -> Result<Shortcut, String> {
    let (key, shift) = match character {
        'a'..='z' => (format!("Key{}", character.to_ascii_uppercase()), false),
        'A'..='Z' => (format!("Key{character}"), true),
        '0'..='9' => (format!("Digit{character}"), false),
        ' ' => ("Space".into(), false),
        ';' | ':' => ("Semicolon".into(), character == ':'),
        '=' | '+' => ("Equal".into(), character == '+'),
        ',' | '<' => ("Comma".into(), character == '<'),
        '-' | '_' => ("Minus".into(), character == '_'),
        '.' | '>' => ("Period".into(), character == '>'),
        '/' | '?' => ("Slash".into(), character == '?'),
        '`' | '~' => ("Backquote".into(), character == '~'),
        '[' | '{' => ("BracketLeft".into(), character == '{'),
        '\\' | '|' => ("Backslash".into(), character == '|'),
        ']' | '}' => ("BracketRight".into(), character == '}'),
        '\'' | '"' => ("Quote".into(), character == '"'),
        '!' | '@' | '#' | '$' | '%' | '^' | '&' | '*' | '(' | ')' => {
            let digit = "!@#$%^&*()".find(character).unwrap();
            (format!("Digit{}", (digit + 1) % 10), true)
        }
        _ => return Err("Captura esa tecla desde Configuracion usando su codigo fisico. Ejemplo: Control+Shift+KeyZ".into()),
    };
    let value = if shift { format!("Shift+{key}") } else { key };
    Shortcut::from_str(&value).map_err(|_| "No se pudo interpretar esa tecla de macOS".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ascii_aliases_keep_shift_and_physical_codes() {
        for (character, code) in [
            ('z', "KeyZ"),
            ('Z', "Shift+KeyZ"),
            ('+', "Shift+Equal"),
            (' ', "Space"),
            ('0', "Digit0"),
            ('!', "Shift+Digit1"),
            (')', "Shift+Digit0"),
        ] {
            assert_eq!(
                parse_character(character).unwrap(),
                Shortcut::from_str(code).unwrap()
            );
        }
        assert!(parse_character('\u{00f1}').is_err());
    }
    #[test]
    fn recommended_combinations_are_distinct_and_leave_system_screenshots_alone() {
        let shortcuts: Vec<_> = [
            "Control+Shift+KeyZ",
            "Control+Shift+KeyX",
            "Control+Shift+KeyA",
            "Control+Shift+KeyC",
        ]
        .map(|value| Shortcut::from_str(value).unwrap())
        .into_iter()
        .collect();
        for (index, shortcut) in shortcuts.iter().enumerate() {
            assert!(!shortcuts[..index].contains(shortcut));
            for native in [
                "Super+Shift+Digit3",
                "Super+Shift+Digit4",
                "Super+Shift+Digit5",
                "Super+Space",
                "Control+Space",
            ] {
                assert_ne!(*shortcut, Shortcut::from_str(native).unwrap());
            }
        }
        assert_eq!(
            Shortcut::from_str("Command+KeyV").unwrap(),
            Shortcut::from_str("Super+KeyV").unwrap()
        );
    }
}
