use fission_core::env::ImeHandler;
use fission_ir::semantics::{
    TextCapitalization, TextFieldValidationState, TextInputAction, TextInputType,
};
use fission_ir::Semantics;
use fission_render::LayoutRect;
use std::sync::{Arc, Mutex};
use winit::window::{ImePurpose, Window};

pub(crate) fn text_edit_command_from_ime_state(
    state: &winit::event::ImeTextState,
) -> Result<fission_core::TextEditCommand, &'static str> {
    let base = fission_core::TextPosition::from_utf16(&state.text, state.selection_start)
        .map_err(|_| "selection start is not a UTF-16 boundary")?;
    let extent = fission_core::TextPosition::from_utf16(&state.text, state.selection_end)
        .map_err(|_| "selection end is not a UTF-16 boundary")?;
    let composing = state
        .composing
        .map(|(start, end)| {
            let start = fission_core::TextPosition::from_utf16(&state.text, start)
                .map_err(|_| "composing start is not a UTF-16 boundary")?;
            let end = fission_core::TextPosition::from_utf16(&state.text, end)
                .map_err(|_| "composing end is not a UTF-16 boundary")?;
            Ok::<_, &'static str>(fission_core::TextRange::from_positions(start, end))
        })
        .transpose()?;

    Ok(fission_core::TextEditCommand::SetValue {
        value: fission_core::TextEditingValue {
            text: state.text.clone(),
            selection: fission_core::TextSelection {
                base,
                extent,
                affinity: fission_core::TextAffinity::Downstream,
            },
            composing,
        },
        source: fission_core::TextEditSource::Ime,
        phase: if state.composing.is_some() {
            fission_core::TextValuePhase::CompositionUpdated
        } else {
            fission_core::TextValuePhase::Committed
        },
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct TextInputConfig {
    pub name: Option<String>,
    pub autofill_group: Option<String>,
    pub text_input_type: TextInputType,
    pub multiline: bool,
    pub text_input_action: TextInputAction,
    pub text_capitalization: TextCapitalization,
    pub read_only: bool,
    pub disabled: bool,
    pub autocorrect: bool,
    pub enable_suggestions: bool,
    pub spell_check: bool,
    pub smart_dashes: bool,
    pub smart_quotes: bool,
    pub autofill_hints: Vec<String>,
    pub ime_purpose: ImePurpose,
    pub accessibility_label: Option<String>,
    pub required: bool,
    pub validation_state: TextFieldValidationState,
    pub validation_message: Option<String>,
}

impl TextInputConfig {
    pub(crate) fn from_semantics(semantics: &Semantics) -> Self {
        Self {
            name: semantics.text_field_name.clone(),
            autofill_group: semantics.autofill_group.clone(),
            text_input_type: semantics.text_input_type,
            multiline: semantics.multiline,
            text_input_action: semantics.text_input_action,
            text_capitalization: semantics.text_capitalization,
            read_only: semantics.read_only,
            disabled: semantics.disabled,
            autocorrect: semantics.autocorrect,
            enable_suggestions: semantics.enable_suggestions,
            spell_check: semantics.spell_check,
            smart_dashes: semantics.smart_dashes,
            smart_quotes: semantics.smart_quotes,
            autofill_hints: semantics.autofill_hints.clone(),
            ime_purpose: ime_purpose_for_semantics(semantics),
            accessibility_label: semantics.label.clone(),
            required: semantics.required,
            validation_state: semantics.validation_state,
            validation_message: semantics.validation_message.clone(),
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
            #[cfg(target_os = "android")]
            crate::android_text_input::update_cursor_area(rect, window.scale_factor() as f32);
        }
    }

    fn set_editing_value(&self, value: &fission_core::TextEditingValue) {
        #[cfg(any(target_arch = "wasm32", target_os = "android", target_os = "ios"))]
        {
            let Ok(base) = value.selection.base.utf16_offset(&value.text) else {
                return;
            };
            let Ok(extent) = value.selection.extent.utf16_offset(&value.text) else {
                return;
            };
            let composing = value.composing.and_then(|range| {
                Some((
                    range.start.utf16_offset(&value.text).ok()?,
                    range.end.utf16_offset(&value.text).ok()?,
                ))
            });
            let state = self.state.lock().expect("ime handler lock poisoned");
            if let Some(window) = state.window.as_ref() {
                window.set_ime_state(winit::event::ImeTextState {
                    text: value.text.clone(),
                    selection_start: base,
                    selection_end: extent,
                    composing,
                });
            }
        }
        #[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
        let _ = value;
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
    #[cfg(target_arch = "wasm32")]
    if let Some(config) = config {
        window.set_web_ime_configuration(web_ime_configuration(config));
        #[cfg(debug_assertions)]
        diagnose_web_unsupported(config);
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    if let Some(config) = config {
        window.set_ime_configuration(mobile_ime_configuration(config));
        #[cfg(all(debug_assertions, target_os = "android"))]
        diagnose_android_unsupported(config);
        #[cfg(all(debug_assertions, target_os = "ios"))]
        diagnose_ios_unsupported(config);
    }
    #[cfg(target_os = "android")]
    crate::android_text_input::configure_autofill(
        config
            .map(|config| config.autofill_hints.as_slice())
            .unwrap_or_default(),
    );
    #[cfg(target_os = "macos")]
    {
        macos::apply_text_input_traits(window, config, mac_view_id);
        #[cfg(debug_assertions)]
        if let Some(config) = config {
            diagnose_macos_unsupported(config);
        }
    }
    #[cfg(all(
        debug_assertions,
        not(target_arch = "wasm32"),
        not(any(target_os = "macos", target_os = "android", target_os = "ios"))
    ))]
    if let Some(config) = config {
        diagnose_unsupported_text_input_config(config);
    }
}

#[cfg(all(
    debug_assertions,
    not(target_arch = "wasm32"),
    not(any(target_os = "macos", target_os = "android", target_os = "ios"))
))]
fn diagnose_unsupported_text_input_config(config: &TextInputConfig) {
    report_unsupported(
        "keyboard_type",
        config.text_input_type != TextInputType::Text,
    );
    report_unsupported(
        "text_input_action",
        config.text_input_action != TextInputAction::Done,
    );
    report_unsupported(
        "text_capitalization",
        config.text_capitalization != TextCapitalization::None,
    );
    report_unsupported("autocorrect=false", !config.autocorrect);
    report_unsupported("enable_suggestions=false", !config.enable_suggestions);
    report_unsupported("spell_check=false", !config.spell_check);
    report_unsupported("smart_dashes=false", !config.smart_dashes);
    report_unsupported("smart_quotes=false", !config.smart_quotes);
    report_unsupported("autofill_hints", !config.autofill_hints.is_empty());
}

#[cfg(debug_assertions)]
fn report_unsupported(name: &'static str, configured: bool) {
    use std::collections::HashSet;
    use std::sync::{Mutex, OnceLock};

    static REPORTED: OnceLock<Mutex<HashSet<(&'static str, &'static str)>>> = OnceLock::new();
    if configured
        && REPORTED
            .get_or_init(|| Mutex::new(HashSet::new()))
            .lock()
            .expect("text input diagnostic lock poisoned")
            .insert((std::env::consts::OS, name))
    {
        log::warn!(
            "Fission text input configuration `{name}` is not implemented by the {} shell adapter; the host default remains active",
            std::env::consts::OS
        );
    }
}

#[cfg(all(debug_assertions, target_os = "macos"))]
fn diagnose_macos_unsupported(config: &TextInputConfig) {
    report_unsupported(
        "keyboard_type",
        config.text_input_type != TextInputType::Text,
    );
    report_unsupported(
        "text_input_action",
        config.text_input_action != TextInputAction::Done,
    );
    report_unsupported(
        "text_capitalization",
        config.text_capitalization != TextCapitalization::None,
    );
    report_unsupported("autofill_hints", !config.autofill_hints.is_empty());
}

#[cfg(all(debug_assertions, target_os = "android"))]
fn diagnose_android_unsupported(config: &TextInputConfig) {
    report_unsupported(
        "spell_check=false with suggestions enabled",
        !config.spell_check && config.enable_suggestions,
    );
    report_unsupported("smart_dashes=false", !config.smart_dashes);
    report_unsupported("smart_quotes=false", !config.smart_quotes);
}

#[cfg(all(debug_assertions, target_os = "ios"))]
fn diagnose_ios_unsupported(config: &TextInputConfig) {
    report_unsupported(
        "enable_suggestions=false with autocorrect enabled",
        !config.enable_suggestions && config.autocorrect,
    );
}

#[cfg(all(debug_assertions, target_arch = "wasm32"))]
fn diagnose_web_unsupported(config: &TextInputConfig) {
    report_unsupported(
        "enable_suggestions=false with autocorrect enabled",
        !config.enable_suggestions && config.autocorrect,
    );
    // Disabling browser autocorrection also disables its smart substitution
    // path. Only diagnose the two finer hints when autocorrection remains on
    // and the browser cannot honor them independently.
    report_unsupported(
        "smart_dashes=false",
        web_smart_hint_needs_advisory(config.autocorrect, config.smart_dashes),
    );
    report_unsupported(
        "smart_quotes=false",
        web_smart_hint_needs_advisory(config.autocorrect, config.smart_quotes),
    );
}

#[cfg(any(target_arch = "wasm32", test))]
fn web_smart_hint_needs_advisory(autocorrect: bool, hint_enabled: bool) -> bool {
    autocorrect && !hint_enabled
}

fn mobile_ime_configuration(config: &TextInputConfig) -> winit::window::ImeConfiguration {
    use winit::window::{ImeAction, ImeCapitalization, ImeConfiguration, ImeInputType};

    let input_type = if config.multiline || config.text_input_type == TextInputType::Multiline {
        ImeInputType::Multiline
    } else {
        match config.text_input_type {
            TextInputType::Text => ImeInputType::Text,
            TextInputType::Multiline => ImeInputType::Multiline,
            TextInputType::Number => ImeInputType::Number,
            TextInputType::EmailAddress => ImeInputType::Email,
            TextInputType::Url => ImeInputType::Url,
            TextInputType::Phone => ImeInputType::Phone,
            TextInputType::Name => ImeInputType::Name,
        }
    };
    let action = match config.text_input_action {
        TextInputAction::Done | TextInputAction::EmergencyCall => ImeAction::Done,
        TextInputAction::Go | TextInputAction::Route | TextInputAction::Join => ImeAction::Go,
        TextInputAction::Search => ImeAction::Search,
        TextInputAction::Send => ImeAction::Send,
        TextInputAction::Next => ImeAction::Next,
        TextInputAction::Previous => ImeAction::Previous,
        TextInputAction::Continue | TextInputAction::Newline => ImeAction::Newline,
    };
    let capitalization = match config.text_capitalization {
        TextCapitalization::None => ImeCapitalization::None,
        TextCapitalization::Characters => ImeCapitalization::Characters,
        TextCapitalization::Words => ImeCapitalization::Words,
        TextCapitalization::Sentences => ImeCapitalization::Sentences,
    };
    ImeConfiguration {
        input_type,
        action,
        capitalization,
        autocorrect: config.autocorrect,
        suggestions: config.enable_suggestions,
        spellcheck: config.spell_check,
        smart_dashes: config.smart_dashes,
        smart_quotes: config.smart_quotes,
        secure: config.ime_purpose == ImePurpose::Password,
        autofill_hints: config.autofill_hints.clone(),
    }
}

#[cfg(target_arch = "wasm32")]
fn web_ime_configuration(config: &TextInputConfig) -> winit::window::WebImeConfiguration {
    let input_mode = match config.text_input_type {
        TextInputType::Text | TextInputType::Multiline | TextInputType::Name => "text",
        TextInputType::Number => "decimal",
        TextInputType::EmailAddress => "email",
        TextInputType::Url => "url",
        TextInputType::Phone => "tel",
    };
    let enter_key_hint = match config.text_input_action {
        TextInputAction::Done => "done",
        TextInputAction::Go | TextInputAction::Route => "go",
        TextInputAction::Search => "search",
        TextInputAction::Send | TextInputAction::EmergencyCall => "send",
        TextInputAction::Next => "next",
        TextInputAction::Previous => "previous",
        TextInputAction::Continue | TextInputAction::Join | TextInputAction::Newline => "enter",
    };
    let autocapitalize = match config.text_capitalization {
        TextCapitalization::None => "none",
        TextCapitalization::Characters => "characters",
        TextCapitalization::Words => "words",
        TextCapitalization::Sentences => "sentences",
    };
    let autocomplete = web_autocomplete(config);
    winit::window::WebImeConfiguration {
        name: config.name.clone().unwrap_or_default(),
        input_mode: input_mode.into(),
        enter_key_hint: enter_key_hint.into(),
        autocomplete,
        autocapitalize: autocapitalize.into(),
        autocorrect: config.autocorrect,
        spellcheck: config.spell_check,
        secure: config.ime_purpose == ImePurpose::Password,
        aria_label: config.accessibility_label.clone().unwrap_or_default(),
        required: config.required,
        invalid: config.validation_state == TextFieldValidationState::Invalid,
        aria_description: config.validation_message.clone().unwrap_or_default(),
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn web_autocomplete(config: &TextInputConfig) -> String {
    let mut autocomplete = Vec::new();
    if let Some(group) = config.autofill_group.as_deref() {
        let normalized = group
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch == '-' {
                    ch
                } else {
                    '-'
                }
            })
            .collect::<String>();
        if !normalized.is_empty() {
            autocomplete.push(format!("section-{normalized}"));
        }
    }
    autocomplete.extend(config.autofill_hints.iter().cloned());
    autocomplete.join(" ")
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
        let Some(view) = ns_view_from_window(window) else {
            clear_text_input_traits(active_view_id.take());
            return;
        };
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

    fn ns_view_from_window(window: &Window) -> Option<id> {
        let handle = window.window_handle().ok()?;
        match handle.as_raw() {
            RawWindowHandle::AppKit(handle) => Some(handle.ns_view.as_ptr() as id),
            _ => None,
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
    use super::{
        active_platform_config, effective_ime_allowed, mobile_ime_configuration,
        text_edit_command_from_ime_state, web_autocomplete, web_smart_hint_needs_advisory,
        TextInputConfig,
    };
    use fission_core::{TextEditCommand, TextEditSource};
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
    fn browser_autofill_keeps_section_identity_and_field_hints() {
        let semantics = Semantics {
            text_field_name: Some("email".into()),
            autofill_group: Some("billing address".into()),
            autofill_hints: vec!["email".into()],
            ..Semantics::default()
        };
        let config = TextInputConfig::from_semantics(&semantics);

        assert_eq!(config.name.as_deref(), Some("email"));
        assert_eq!(web_autocomplete(&config), "section-billing-address email");
    }

    #[test]
    fn disabled_web_autocorrection_also_satisfies_disabled_smart_hints() {
        assert!(!web_smart_hint_needs_advisory(false, false));
        assert!(!web_smart_hint_needs_advisory(false, true));
        assert!(!web_smart_hint_needs_advisory(true, true));
        assert!(web_smart_hint_needs_advisory(true, false));
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

    #[test]
    fn complete_platform_state_converts_utf16_selection_and_composition() {
        let command = text_edit_command_from_ime_state(&winit::event::ImeTextState {
            text: "A🙂世".into(),
            selection_start: 3,
            selection_end: 4,
            composing: Some((1, 4)),
        })
        .expect("valid platform text state");

        let TextEditCommand::SetValue {
            value,
            source,
            phase,
        } = command
        else {
            panic!("platform state must become a complete SetValue command");
        };
        assert_eq!(source, TextEditSource::Ime);
        assert_eq!(phase, fission_core::TextValuePhase::CompositionUpdated);
        assert_eq!(value.selection.base.utf8_offset(), 5);
        assert_eq!(value.selection.extent.utf8_offset(), 8);
        let composing = value.composing.expect("composition");
        assert_eq!(composing.start.utf8_offset(), 1);
        assert_eq!(composing.end.utf8_offset(), 8);
    }

    #[test]
    fn complete_platform_state_rejects_surrogate_splitting_offsets() {
        let result = text_edit_command_from_ime_state(&winit::event::ImeTextState {
            text: "A🙂B".into(),
            selection_start: 2,
            selection_end: 2,
            composing: None,
        });
        assert_eq!(
            result.unwrap_err(),
            "selection start is not a UTF-16 boundary"
        );
    }

    #[test]
    fn mobile_configuration_maps_keyboard_actions_and_secure_autofill() {
        use fission_ir::semantics::TextCapitalization;
        use winit::window::{ImeAction, ImeCapitalization, ImeInputType};

        for (action, expected) in [
            (TextInputAction::Done, ImeAction::Done),
            (TextInputAction::Go, ImeAction::Go),
            (TextInputAction::Search, ImeAction::Search),
            (TextInputAction::Send, ImeAction::Send),
            (TextInputAction::Next, ImeAction::Next),
            (TextInputAction::Previous, ImeAction::Previous),
            (TextInputAction::Newline, ImeAction::Newline),
        ] {
            let config = TextInputConfig {
                text_input_action: action,
                ..TextInputConfig::default()
            };
            assert_eq!(mobile_ime_configuration(&config).action, expected);
        }

        let config = TextInputConfig {
            text_input_type: TextInputType::EmailAddress,
            text_capitalization: TextCapitalization::Words,
            ime_purpose: ImePurpose::Password,
            autofill_hints: vec!["username".into(), "current-password".into()],
            autocorrect: false,
            enable_suggestions: false,
            spell_check: false,
            smart_dashes: false,
            smart_quotes: false,
            ..TextInputConfig::default()
        };
        let mapped = mobile_ime_configuration(&config);
        assert_eq!(mapped.input_type, ImeInputType::Email);
        assert_eq!(mapped.capitalization, ImeCapitalization::Words);
        assert!(mapped.secure);
        assert_eq!(mapped.autofill_hints, config.autofill_hints);
        assert!(!mapped.autocorrect);
        assert!(!mapped.suggestions);
        assert!(!mapped.spellcheck);
        assert!(!mapped.smart_dashes);
        assert!(!mapped.smart_quotes);
    }
}
