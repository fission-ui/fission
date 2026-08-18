use crate::event::{MOD_ALT, MOD_CTRL, MOD_SUPER};

/// Host text-editing conventions used by platform input controllers.
///
/// This is supplied by the runtime rather than inferred from the Rust target:
/// a WebAssembly application can be running on either an Apple or non-Apple
/// host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextEditingConvention {
    /// Control is the primary shortcut and word-navigation modifier.
    #[default]
    Standard,
    /// Command is the primary shortcut, Option navigates by word, and the
    /// conventional Control-based line-editing commands are available.
    Apple,
}

impl TextEditingConvention {
    pub const fn is_apple(self) -> bool {
        matches!(self, Self::Apple)
    }

    pub const fn primary_shortcut_modifier(self) -> u8 {
        match self {
            Self::Standard => MOD_CTRL,
            Self::Apple => MOD_SUPER,
        }
    }

    pub const fn has_primary_shortcut(self, modifiers: u8) -> bool {
        (modifiers & self.primary_shortcut_modifier()) != 0
    }

    pub const fn has_word_modifier(self, modifiers: u8) -> bool {
        match self {
            Self::Standard => (modifiers & MOD_CTRL) != 0,
            Self::Apple => (modifiers & MOD_ALT) != 0,
        }
    }

    /// Ctrl+Alt represents AltGr on common non-Apple keyboard layouts. The
    /// resulting character is text input, not a Control shortcut.
    pub const fn is_alt_gr(self, modifiers: u8) -> bool {
        matches!(self, Self::Standard)
            && (modifiers & (MOD_CTRL | MOD_ALT)) == (MOD_CTRL | MOD_ALT)
            && (modifiers & MOD_SUPER) == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conventions_define_host_shortcuts_without_compile_target_checks() {
        assert!(TextEditingConvention::Apple.has_primary_shortcut(MOD_SUPER));
        assert!(!TextEditingConvention::Apple.has_primary_shortcut(MOD_CTRL));
        assert!(TextEditingConvention::Apple.has_word_modifier(MOD_ALT));

        assert!(TextEditingConvention::Standard.has_primary_shortcut(MOD_CTRL));
        assert!(!TextEditingConvention::Standard.has_primary_shortcut(MOD_SUPER));
        assert!(TextEditingConvention::Standard.has_word_modifier(MOD_CTRL));
    }

    #[test]
    fn only_standard_ctrl_alt_is_alt_gr() {
        assert!(TextEditingConvention::Standard.is_alt_gr(MOD_CTRL | MOD_ALT));
        assert!(!TextEditingConvention::Apple.is_alt_gr(MOD_CTRL | MOD_ALT));
        assert!(!TextEditingConvention::Standard.is_alt_gr(MOD_CTRL));
        assert!(!TextEditingConvention::Standard.is_alt_gr(MOD_CTRL | MOD_ALT | MOD_SUPER));
    }
}
