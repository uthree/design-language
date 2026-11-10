//! A button's tooltip names its keys, written for the platform and read from the keymap.

use atelier::binding_label;
use gpui::{KeyBinding, NoAction};

#[test]
fn a_binding_is_written_as_the_platform_writes_it() {
    let save = KeyBinding::new("secondary-shift-s", NoAction, None);
    let chord = KeyBinding::new("ctrl-k ctrl-s", NoAction, None);
    if cfg!(target_os = "macos") {
        assert_eq!(binding_label(&save).as_ref(), "⇧⌘S");
        assert_eq!(binding_label(&chord).as_ref(), "⌃K ⌃S");
    } else {
        assert_eq!(binding_label(&save).as_ref(), "Ctrl+Shift+S");
        assert_eq!(binding_label(&chord).as_ref(), "Ctrl+K Ctrl+S");
    }
}
