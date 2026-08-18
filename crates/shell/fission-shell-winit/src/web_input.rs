use fission_core::input::TextEditingConvention;

/// Browser behavior which a Web application deliberately delegates to the browser.
///
/// Fission owns input by default. An empty policy therefore keeps every browser
/// default suppressed while still delivering the corresponding event to Fission.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BrowserDefaults(u8);

impl BrowserDefaults {
    pub const NONE: Self = Self(0);
    pub const KEYBOARD: Self = Self(1 << 0);
    pub const POINTER: Self = Self(1 << 1);
    pub const TOUCH: Self = Self(1 << 2);
    pub const WHEEL: Self = Self(1 << 3);
    pub const CONTEXT_MENU: Self = Self(1 << 4);
    pub const CLIPBOARD: Self = Self(1 << 5);

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl std::ops::BitOr for BrowserDefaults {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for BrowserDefaults {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn to_winit(defaults: BrowserDefaults) -> winit::platform::web::BrowserDefaults {
    use winit::platform::web::BrowserDefaults as WinitDefaults;

    let mut result = WinitDefaults::NONE;
    for (fission, winit) in [
        (BrowserDefaults::KEYBOARD, WinitDefaults::KEYBOARD),
        (BrowserDefaults::POINTER, WinitDefaults::POINTER),
        (BrowserDefaults::TOUCH, WinitDefaults::TOUCH),
        (BrowserDefaults::WHEEL, WinitDefaults::WHEEL),
        (BrowserDefaults::CONTEXT_MENU, WinitDefaults::CONTEXT_MENU),
        (BrowserDefaults::CLIPBOARD, WinitDefaults::CLIPBOARD),
    ] {
        if defaults.contains(fission) {
            result |= winit;
        }
    }
    result
}

pub(crate) fn host_text_editing_convention() -> TextEditingConvention {
    #[cfg(target_arch = "wasm32")]
    {
        let global = js_sys::global();
        let navigator = js_sys::Reflect::get(&global, &"navigator".into()).ok();
        let platform = navigator
            .as_ref()
            .and_then(|navigator| js_sys::Reflect::get(navigator, &"platform".into()).ok())
            .and_then(|platform| platform.as_string());
        let user_agent = navigator
            .as_ref()
            .and_then(|navigator| js_sys::Reflect::get(navigator, &"userAgent".into()).ok())
            .and_then(|user_agent| user_agent.as_string());
        return convention_for_browser_host(platform.as_deref(), user_agent.as_deref());
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        if cfg!(any(target_os = "macos", target_os = "ios")) {
            TextEditingConvention::Apple
        } else {
            TextEditingConvention::Standard
        }
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn convention_for_browser_host(
    platform: Option<&str>,
    user_agent: Option<&str>,
) -> TextEditingConvention {
    let is_apple = platform.into_iter().chain(user_agent).any(|value| {
        ["mac", "iphone", "ipad", "ipod"]
            .iter()
            .any(|needle| value.to_ascii_lowercase().contains(needle))
    });
    if is_apple {
        TextEditingConvention::Apple
    } else {
        TextEditingConvention::Standard
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_host_selects_apple_conventions_without_using_wasm_target_os() {
        assert_eq!(
            convention_for_browser_host(Some("MacIntel"), None),
            TextEditingConvention::Apple
        );
        assert_eq!(
            convention_for_browser_host(None, Some("Mozilla/5.0 (iPhone)")),
            TextEditingConvention::Apple
        );
        assert_eq!(
            convention_for_browser_host(Some("Win32"), None),
            TextEditingConvention::Standard
        );
    }

    #[test]
    fn browser_defaults_are_composable_and_deny_by_default() {
        assert!(!BrowserDefaults::NONE.contains(BrowserDefaults::KEYBOARD));
        let editing = BrowserDefaults::KEYBOARD | BrowserDefaults::CLIPBOARD;
        assert!(editing.contains(BrowserDefaults::KEYBOARD));
        assert!(editing.contains(BrowserDefaults::CLIPBOARD));
        assert!(!editing.contains(BrowserDefaults::CONTEXT_MENU));
    }
}
