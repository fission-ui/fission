use fission_core::env::ImeHandler;
use fission_ir::semantics::{TextInputAction, TextInputType};
use fission_ir::Semantics;
use fission_render::LayoutRect;
use std::sync::{Arc, Mutex};
use winit::window::{ImePurpose, Window};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct TextInputConfig {
    pub text_input_type: TextInputType,
    pub text_input_action: TextInputAction,
    pub read_only: bool,
    pub disabled: bool,
    pub autocorrect: bool,
    pub enable_suggestions: bool,
    pub spell_check: bool,
    pub smart_dashes: bool,
    pub smart_quotes: bool,
    pub autofill_hints: Vec<String>,
    pub ime_purpose: ImePurpose,
}

impl TextInputConfig {
    pub(crate) fn from_semantics(semantics: &Semantics) -> Self {
        Self {
            text_input_type: semantics.text_input_type,
            text_input_action: semantics.text_input_action,
            read_only: semantics.read_only,
            disabled: semantics.disabled,
            autocorrect: semantics.autocorrect,
            enable_suggestions: semantics.enable_suggestions,
            spell_check: semantics.spell_check,
            smart_dashes: semantics.smart_dashes,
            smart_quotes: semantics.smart_quotes,
            autofill_hints: semantics.autofill_hints.clone(),
            ime_purpose: ime_purpose_for_semantics(semantics),
        }
    }

    fn allows_platform_editing(&self) -> bool {
        !self.read_only && !self.disabled
    }
}

fn ime_purpose_for_semantics(semantics: &Semantics) -> ImePurpose {
    let password_like_hint = semantics.autofill_hints.iter().any(|hint| {
        hint.trim()
            .chars()
            .filter_map(|ch| match ch {
                '-' | '_' => None,
                _ => Some(ch.to_ascii_lowercase()),
            })
            .collect::<String>()
            .contains("password")
    });
    if semantics.masked || password_like_hint {
        ImePurpose::Password
    } else {
        ImePurpose::Normal
    }
}

#[derive(Default)]
struct ImeHandlerState {
    window: Option<Arc<Window>>,
    text_input_config: Option<TextInputConfig>,
    ime_allowed_requested: bool,
    #[cfg(target_os = "macos")]
    mac_view_id: Option<usize>,
}

#[derive(Default)]
pub struct DesktopImeHandler {
    state: Mutex<ImeHandlerState>,
}

impl DesktopImeHandler {
    pub fn set_window(&self, window: Option<Arc<Window>>) {
        let mut state = self.state.lock().expect("ime handler lock poisoned");
        state.window = window;
        sync_text_input_config(&mut state);
    }

    pub fn set_text_input_config(&self, config: Option<TextInputConfig>) {
        let mut state = self.state.lock().expect("ime handler lock poisoned");
        if state.text_input_config == config {
            return;
        }
        state.text_input_config = config;
        sync_text_input_config(&mut state);
    }
}

impl Drop for DesktopImeHandler {
    fn drop(&mut self) {
        #[cfg(target_os = "macos")]
        if let Ok(mut state) = self.state.lock() {
            macos::clear_text_input_traits(state.mac_view_id.take());
        }
    }
}

impl ImeHandler for DesktopImeHandler {
    fn set_ime_allowed(&self, allowed: bool) {
        let mut state = self.state.lock().expect("ime handler lock poisoned");
        if state.ime_allowed_requested == allowed {
            return;
        }
        state.ime_allowed_requested = allowed;
        sync_text_input_config(&mut state);
    }

    fn set_ime_cursor_area(&self, rect: LayoutRect) {
        let state = self.state.lock().expect("ime handler lock poisoned");
        if !effective_ime_allowed(
            state.ime_allowed_requested,
            state.text_input_config.as_ref(),
        ) {
            return;
        }
        if let Some(window) = state.window.as_ref() {
            // Position relative to window
            window.set_ime_cursor_area(
                winit::dpi::PhysicalPosition::new(rect.x() as f64, rect.y() as f64),
                winit::dpi::PhysicalSize::new(rect.width() as u32, rect.height() as u32),
            );
        }
    }
}

fn sync_text_input_config(state: &mut ImeHandlerState) {
    if let Some(window) = state.window.as_ref() {
        window.set_ime_allowed(effective_ime_allowed(
            state.ime_allowed_requested,
            state.text_input_config.as_ref(),
        ));
        apply_text_input_config(
            window,
            active_platform_config(state.text_input_config.as_ref()),
            #[cfg(target_os = "macos")]
            &mut state.mac_view_id,
        );
    } else {
        #[cfg(target_os = "macos")]
        macos::clear_text_input_traits(state.mac_view_id.take());
    }
}

fn effective_ime_allowed(requested: bool, config: Option<&TextInputConfig>) -> bool {
    requested
        && config
            .map(TextInputConfig::allows_platform_editing)
            .unwrap_or(true)
}

fn active_platform_config(config: Option<&TextInputConfig>) -> Option<&TextInputConfig> {
    config.filter(|config| config.allows_platform_editing())
}

fn apply_text_input_config(
    window: &Window,
    config: Option<&TextInputConfig>,
    #[cfg(target_os = "macos")] mac_view_id: &mut Option<usize>,
) {
    window.set_ime_purpose(config.map(|config| config.ime_purpose).unwrap_or_default());
    #[cfg(target_os = "macos")]
    macos::apply_text_input_traits(window, config, mac_view_id);
}

#[cfg(target_os = "macos")]
mod macos {
    use super::TextInputConfig;
    use cocoa::base::{id, nil};
    use objc::runtime::{class_addMethod, object_getClass, Class, Object, Sel};
    use objc::{msg_send, sel, sel_impl};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::collections::{HashMap, HashSet};
    use std::sync::{Mutex, OnceLock};
    use winit::window::Window;

    const TRAIT_DEFAULT: isize = 0;
    const TRAIT_NO: isize = 1;
    const TRAIT_YES: isize = 2;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct MacTextInputTraits {
        autocorrection_type: isize,
        spell_checking_type: isize,
        smart_quotes_type: isize,
        smart_dashes_type: isize,
        text_completion_type: isize,
    }

    impl Default for MacTextInputTraits {
        fn default() -> Self {
            Self {
                autocorrection_type: TRAIT_DEFAULT,
                spell_checking_type: TRAIT_DEFAULT,
                smart_quotes_type: TRAIT_DEFAULT,
                smart_dashes_type: TRAIT_DEFAULT,
                text_completion_type: TRAIT_DEFAULT,
            }
        }
    }

    impl From<&TextInputConfig> for MacTextInputTraits {
        fn from(config: &TextInputConfig) -> Self {
            Self {
                autocorrection_type: trait_flag(config.autocorrect),
                spell_checking_type: trait_flag(config.spell_check),
                smart_quotes_type: trait_flag(config.smart_quotes),
                smart_dashes_type: trait_flag(config.smart_dashes),
                text_completion_type: trait_flag(config.enable_suggestions),
            }
        }
    }

    pub(super) fn apply_text_input_traits(
        window: &Window,
        config: Option<&TextInputConfig>,
        active_view_id: &mut Option<usize>,
    ) {
        let view = ns_view_from_window(window);
        ensure_trait_bridge(view);

        let view_id = view as usize;
        if let Some(previous_view_id) = active_view_id.replace(view_id) {
            if previous_view_id != view_id {
                traits_by_view()
                    .lock()
                    .expect("macos text input traits lock poisoned")
                    .remove(&previous_view_id);
            }
        }

        let mut traits = traits_by_view()
            .lock()
            .expect("macos text input traits lock poisoned");
        if let Some(config) = config {
            traits.insert(view_id, MacTextInputTraits::from(config));
        } else {
            traits.remove(&view_id);
        }
        drop(traits);

        unsafe {
            let input_context: id = msg_send![view, inputContext];
            if input_context != nil {
                let _: () = msg_send![input_context, activate];
                let _: () = msg_send![input_context, invalidateCharacterCoordinates];
            }
        }
    }

    pub(super) fn clear_text_input_traits(view_id: Option<usize>) {
        if let Some(view_id) = view_id {
            traits_by_view()
                .lock()
                .expect("macos text input traits lock poisoned")
                .remove(&view_id);
        }
    }

    fn ns_view_from_window(window: &Window) -> id {
        let handle = window
            .window_handle()
            .expect("window handle unavailable on macOS");
        match handle.as_raw() {
            RawWindowHandle::AppKit(handle) => handle.ns_view.as_ptr() as id,
            other => panic!("expected AppKit window handle, got {other:?}"),
        }
    }

    fn ensure_trait_bridge(view: id) {
        // Winit's AppKit view does not expose these optional traits, so we add
        // lightweight getters on its runtime class and back them with our map.
        let class = unsafe { object_getClass(view.cast::<Object>()) as *mut Class };
        let class_id = class as usize;
        let mut installed = installed_classes()
            .lock()
            .expect("macos text input bridge lock poisoned");
        if !installed.insert(class_id) {
            return;
        }

        unsafe {
            let encoding = b"q@:\0".as_ptr().cast();
            let _ = class_addMethod(
                class,
                sel!(autocorrectionType),
                method_imp(autocorrection_type),
                encoding,
            );
            let _ = class_addMethod(
                class,
                sel!(spellCheckingType),
                method_imp(spell_checking_type),
                encoding,
            );
            let _ = class_addMethod(
                class,
                sel!(smartQuotesType),
                method_imp(smart_quotes_type),
                encoding,
            );
            let _ = class_addMethod(
                class,
                sel!(smartDashesType),
                method_imp(smart_dashes_type),
                encoding,
            );
            let _ = class_addMethod(
                class,
                sel!(textCompletionType),
                method_imp(text_completion_type),
                encoding,
            );
        }
    }

    fn traits_by_view() -> &'static Mutex<HashMap<usize, MacTextInputTraits>> {
        static TRAITS: OnceLock<Mutex<HashMap<usize, MacTextInputTraits>>> = OnceLock::new();
        TRAITS.get_or_init(|| Mutex::new(HashMap::new()))
    }

    fn installed_classes() -> &'static Mutex<HashSet<usize>> {
        static INSTALLED: OnceLock<Mutex<HashSet<usize>>> = OnceLock::new();
        INSTALLED.get_or_init(|| Mutex::new(HashSet::new()))
    }

    fn trait_flag(enabled: bool) -> isize {
        if enabled {
            TRAIT_YES
        } else {
            TRAIT_NO
        }
    }

    fn method_imp(func: unsafe extern "C" fn(&Object, Sel) -> isize) -> objc::runtime::Imp {
        unsafe { std::mem::transmute(func) }
    }

    fn traits_for(view: &Object) -> MacTextInputTraits {
        traits_by_view()
            .lock()
            .expect("macos text input traits lock poisoned")
            .get(&(view as *const Object as usize))
            .copied()
            .unwrap_or_default()
    }

    unsafe extern "C" fn autocorrection_type(view: &Object, _: Sel) -> isize {
        traits_for(view).autocorrection_type
    }

    unsafe extern "C" fn spell_checking_type(view: &Object, _: Sel) -> isize {
        traits_for(view).spell_checking_type
    }

    unsafe extern "C" fn smart_quotes_type(view: &Object, _: Sel) -> isize {
        traits_for(view).smart_quotes_type
    }

    unsafe extern "C" fn smart_dashes_type(view: &Object, _: Sel) -> isize {
        traits_for(view).smart_dashes_type
    }

    unsafe extern "C" fn text_completion_type(view: &Object, _: Sel) -> isize {
        traits_for(view).text_completion_type
    }
}

#[cfg(test)]
mod tests {
    use super::{active_platform_config, effective_ime_allowed, TextInputConfig};
    use fission_ir::semantics::{TextInputAction, TextInputType};
    use fission_ir::Semantics;
    use winit::window::ImePurpose;

    #[test]
    fn text_input_config_copies_runtime_semantics() {
        let semantics = Semantics {
            masked: false,
            text_input_type: TextInputType::EmailAddress,
            text_input_action: TextInputAction::Search,
            read_only: true,
            autocorrect: false,
            enable_suggestions: false,
            spell_check: false,
            smart_dashes: false,
            smart_quotes: true,
            autofill_hints: vec!["email".into()],
            ..Semantics::default()
        };

        let config = TextInputConfig::from_semantics(&semantics);
        assert_eq!(config.text_input_type, TextInputType::EmailAddress);
        assert_eq!(config.text_input_action, TextInputAction::Search);
        assert!(config.read_only);
        assert!(!config.disabled);
        assert!(!config.autocorrect);
        assert!(!config.enable_suggestions);
        assert!(!config.spell_check);
        assert!(!config.smart_dashes);
        assert!(config.smart_quotes);
        assert_eq!(config.autofill_hints, ["email"]);
        assert_eq!(config.ime_purpose, ImePurpose::Normal);
    }

    #[test]
    fn password_autofill_hints_set_password_purpose() {
        let semantics = Semantics {
            autofill_hints: vec!["new-password".into()],
            ..Semantics::default()
        };

        let config = TextInputConfig::from_semantics(&semantics);
        assert_eq!(config.ime_purpose, ImePurpose::Password);
    }

    #[test]
    fn platform_editing_is_disabled_for_non_editable_fields() {
        let read_only = TextInputConfig::from_semantics(&Semantics {
            read_only: true,
            ..Semantics::default()
        });
        let disabled = TextInputConfig::from_semantics(&Semantics {
            disabled: true,
            ..Semantics::default()
        });
        let editable = TextInputConfig::from_semantics(&Semantics::default());

        assert!(!read_only.allows_platform_editing());
        assert!(!disabled.allows_platform_editing());
        assert!(editable.allows_platform_editing());

        assert!(!effective_ime_allowed(true, Some(&read_only)));
        assert!(!effective_ime_allowed(true, Some(&disabled)));
        assert!(effective_ime_allowed(true, Some(&editable)));
        assert!(!effective_ime_allowed(false, Some(&editable)));
        assert!(effective_ime_allowed(true, None));

        assert!(active_platform_config(Some(&read_only)).is_none());
        assert!(active_platform_config(Some(&disabled)).is_none());
        assert!(active_platform_config(Some(&editable)).is_some());
    }

    #[test]
    fn ime_cursor_updates_follow_effective_editability() {
        let read_only = TextInputConfig::from_semantics(&Semantics {
            read_only: true,
            ..Semantics::default()
        });
        let editable = TextInputConfig::from_semantics(&Semantics::default());

        assert!(!effective_ime_allowed(true, Some(&read_only)));
        assert!(effective_ime_allowed(true, Some(&editable)));
    }
}
