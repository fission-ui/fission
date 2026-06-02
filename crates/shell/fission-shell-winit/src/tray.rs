use anyhow::{Context, Result};
use fission_core::internal::BuildCtx;
use fission_core::{
    ActionEnvelope, ActionRegistry, BuildCtxHandle, Env, GlobalState, Runtime, View, ViewHandle,
    WidgetId,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItemBuilder, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconEvent};
use winit::event_loop::EventLoopProxy;
use winit::window::Window;

use crate::Pipeline;
use fission_test_driver::TestEvent;

/// What the desktop shell should do when the main window receives a close request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowCloseBehavior {
    /// Close the process when the window is closed.
    Exit,
    /// Hide the main window while keeping the app and tray icon alive.
    HideToTray,
}

impl Default for WindowCloseBehavior {
    fn default() -> Self {
        Self::Exit
    }
}

/// What the desktop shell should do when the tray icon itself is activated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrayActivateBehavior {
    None,
    ShowMainWindow,
    ToggleMainWindow,
}

impl Default for TrayActivateBehavior {
    fn default() -> Self {
        Self::ToggleMainWindow
    }
}

/// Built-in shell actions that can be used from a tray menu.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrayHostAction {
    ShowMainWindow,
    HideMainWindow,
    ToggleMainWindow,
    QuitApp,
}

/// Icon source used when creating the OS tray/status item.
#[derive(Clone, Debug)]
pub enum TrayIconSource {
    Rgba {
        rgba: Vec<u8>,
        width: u32,
        height: u32,
    },
    PngBytes(Vec<u8>),
    Path(PathBuf),
}

impl TrayIconSource {
    pub fn rgba(rgba: Vec<u8>, width: u32, height: u32) -> Self {
        Self::Rgba {
            rgba,
            width,
            height,
        }
    }

    pub fn png_bytes(bytes: impl Into<Vec<u8>>) -> Self {
        Self::PngBytes(bytes.into())
    }

    pub fn path(path: impl Into<PathBuf>) -> Self {
        Self::Path(path.into())
    }
}

/// Action stored behind a tray menu entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrayMenuAction {
    Host(TrayHostAction),
    App(ActionEnvelope),
}

impl TrayMenuAction {
    pub fn host(action: TrayHostAction) -> Self {
        Self::Host(action)
    }

    pub fn app(action: ActionEnvelope) -> Self {
        Self::App(action)
    }
}

/// A single tray menu item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrayMenuItem {
    pub label: String,
    pub enabled: bool,
    pub action: TrayMenuAction,
}

impl TrayMenuItem {
    pub fn new(label: impl Into<String>, action: TrayMenuAction) -> Self {
        Self {
            label: label.into(),
            enabled: true,
            action,
        }
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

/// Entries in a tray menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrayMenuEntry {
    Item(TrayMenuItem),
    Separator,
}

/// A platform-neutral menu description produced by a tray menu builder.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrayMenu {
    entries: Vec<TrayMenuEntry>,
}

impl TrayMenu {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn item(mut self, label: impl Into<String>, action: TrayMenuAction) -> Self {
        self.entries
            .push(TrayMenuEntry::Item(TrayMenuItem::new(label, action)));
        self
    }

    pub fn disabled_item(mut self, label: impl Into<String>, action: TrayMenuAction) -> Self {
        self.entries.push(TrayMenuEntry::Item(
            TrayMenuItem::new(label, action).disabled(),
        ));
        self
    }

    pub fn separator(mut self) -> Self {
        self.entries.push(TrayMenuEntry::Separator);
        self
    }

    pub fn push(&mut self, entry: TrayMenuEntry) {
        self.entries.push(entry);
    }

    pub fn entries(&self) -> &[TrayMenuEntry] {
        &self.entries
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn with_default_quit(mut self) -> Self {
        if !self.entries.is_empty() {
            self.entries.push(TrayMenuEntry::Separator);
        }
        self.entries.push(TrayMenuEntry::Item(TrayMenuItem::new(
            "Exit",
            TrayMenuAction::host(TrayHostAction::QuitApp),
        )));
        self
    }
}

/// A Fission tray menu builder. It participates in the same state/view/action model as
/// visual widgets but produces a native tray menu model instead of a visual node tree.
pub trait TrayMenuBuilder<S: GlobalState>: Send + Sync + 'static {
    fn menu(&self, ctx: BuildCtxHandle<S>, view: ViewHandle<S>) -> TrayMenu;
}

impl<S, F> TrayMenuBuilder<S> for F
where
    S: GlobalState,
    F: Fn(BuildCtxHandle<S>, ViewHandle<S>) -> TrayMenu + Send + Sync + 'static,
{
    fn menu(&self, ctx: BuildCtxHandle<S>, view: ViewHandle<S>) -> TrayMenu {
        self(ctx, view)
    }
}

pub struct TrayConfig<S: GlobalState> {
    pub icon: TrayIconSource,
    pub tooltip: Option<String>,
    pub title: Option<String>,
    pub icon_is_template: bool,
    pub close_behavior: WindowCloseBehavior,
    pub activate_behavior: TrayActivateBehavior,
    pub menu_on_left_click: bool,
    pub menu_on_right_click: bool,
    pub include_default_quit: bool,
    pub menu: Option<Arc<dyn TrayMenuBuilder<S>>>,
}

impl<S: GlobalState> Clone for TrayConfig<S> {
    fn clone(&self) -> Self {
        Self {
            icon: self.icon.clone(),
            tooltip: self.tooltip.clone(),
            title: self.title.clone(),
            icon_is_template: self.icon_is_template,
            close_behavior: self.close_behavior,
            activate_behavior: self.activate_behavior,
            menu_on_left_click: self.menu_on_left_click,
            menu_on_right_click: self.menu_on_right_click,
            include_default_quit: self.include_default_quit,
            menu: self.menu.clone(),
        }
    }
}

impl<S: GlobalState> TrayConfig<S> {
    pub fn new(icon: TrayIconSource) -> Self {
        Self {
            icon,
            tooltip: None,
            title: None,
            icon_is_template: false,
            close_behavior: WindowCloseBehavior::Exit,
            activate_behavior: TrayActivateBehavior::ToggleMainWindow,
            menu_on_left_click: false,
            menu_on_right_click: true,
            include_default_quit: true,
            menu: None,
        }
    }

    pub fn tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn icon_template(mut self, icon_is_template: bool) -> Self {
        self.icon_is_template = icon_is_template;
        self
    }

    pub fn close_behavior(mut self, behavior: WindowCloseBehavior) -> Self {
        self.close_behavior = behavior;
        self
    }

    pub fn activate_behavior(mut self, behavior: TrayActivateBehavior) -> Self {
        self.activate_behavior = behavior;
        self
    }

    pub fn menu_on_left_click(mut self, enabled: bool) -> Self {
        self.menu_on_left_click = enabled;
        self
    }

    pub fn menu_on_right_click(mut self, enabled: bool) -> Self {
        self.menu_on_right_click = enabled;
        self
    }

    pub fn include_default_quit(mut self, include: bool) -> Self {
        self.include_default_quit = include;
        self
    }

    pub fn menu<M>(mut self, menu: M) -> Self
    where
        M: TrayMenuBuilder<S>,
    {
        self.menu = Some(Arc::new(menu));
        self
    }
}

pub(crate) enum TrayRuntimeEvent {
    TrayIcon(TrayIconEvent),
    Menu(MenuEvent),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct TrayEventOutcome {
    pub(crate) redraw: bool,
    pub(crate) quit: bool,
}

pub(crate) struct ActiveTray<S: GlobalState> {
    config: TrayConfig<S>,
    _tray_icon: TrayIcon,
    _menu: Menu,
    actions_by_menu_id: HashMap<String, TrayMenuAction>,
}

impl<S: GlobalState> ActiveTray<S> {
    pub(crate) fn close_behavior(&self) -> WindowCloseBehavior {
        self.config.close_behavior
    }

    pub(crate) fn build(config: TrayConfig<S>) -> Result<Self> {
        let icon = load_icon(&config.icon)?;
        let tray_menu = fallback_tray_menu(&config);
        let (menu, actions_by_menu_id) = native_menu_from_tray_menu(&tray_menu)?;
        let mut builder = TrayIconBuilder::new()
            .with_icon(icon)
            .with_menu(Box::new(menu.clone()))
            .with_icon_as_template(config.icon_is_template)
            .with_menu_on_left_click(config.menu_on_left_click)
            .with_menu_on_right_click(config.menu_on_right_click);
        if let Some(tooltip) = &config.tooltip {
            builder = builder.with_tooltip(tooltip);
        }
        if let Some(title) = &config.title {
            builder = builder.with_title(title);
        }
        let tray_icon = builder
            .build()
            .context("failed to create Fission tray icon")?;
        Ok(Self {
            config,
            _tray_icon: tray_icon,
            _menu: menu,
            actions_by_menu_id,
        })
    }

    pub(crate) fn refresh_menu(
        &mut self,
        runtime: &Runtime,
        env: &Env,
        pipeline: &Pipeline,
    ) -> Result<ActionRegistry<S>> {
        let (tray_menu, registry) = build_tray_menu(&self.config, runtime, env, pipeline)?;
        let (menu, actions_by_menu_id) = native_menu_from_tray_menu(&tray_menu)?;
        self._tray_icon.set_menu(Some(Box::new(menu.clone())));
        self._menu = menu;
        self.actions_by_menu_id = actions_by_menu_id;
        Ok(registry)
    }

    pub(crate) fn handle_event(
        &self,
        event: TrayRuntimeEvent,
        window: &Window,
        runtime: &mut Runtime,
    ) -> Result<TrayEventOutcome> {
        match event {
            TrayRuntimeEvent::TrayIcon(event) => {
                if should_activate_from_tray_event(&event) {
                    apply_activate_behavior(self.config.activate_behavior, window);
                }
                Ok(TrayEventOutcome::default())
            }
            TrayRuntimeEvent::Menu(event) => {
                let Some(action) = self.actions_by_menu_id.get(event.id.as_ref()).cloned() else {
                    return Ok(TrayEventOutcome::default());
                };
                match action {
                    TrayMenuAction::Host(action) => {
                        if action == TrayHostAction::QuitApp {
                            return Ok(TrayEventOutcome {
                                redraw: false,
                                quit: true,
                            });
                        }
                        apply_host_action(action, window);
                        Ok(TrayEventOutcome::default())
                    }
                    TrayMenuAction::App(action) => {
                        runtime.dispatch(action, WidgetId::from_u128(0))?;
                        Ok(TrayEventOutcome {
                            redraw: true,
                            quit: false,
                        })
                    }
                }
            }
        }
    }
}

pub(crate) fn install_event_forwarders(
    proxy: EventLoopProxy<TestEvent>,
) -> std::sync::mpsc::Receiver<TrayRuntimeEvent> {
    let (tx, rx) = std::sync::mpsc::channel();
    let proxy = Arc::new(std::sync::Mutex::new(proxy));
    let tray_tx = tx.clone();
    let tray_proxy = proxy.clone();
    TrayIconEvent::set_event_handler(Some(move |event| {
        let _ = tray_tx.send(TrayRuntimeEvent::TrayIcon(event));
        if let Ok(proxy) = tray_proxy.lock() {
            let _ = proxy.send_event(TestEvent::Wake);
        }
    }));
    let menu_tx = tx;
    tray_icon::menu::MenuEvent::set_event_handler(Some(move |event| {
        let _ = menu_tx.send(TrayRuntimeEvent::Menu(event));
        if let Ok(proxy) = proxy.lock() {
            let _ = proxy.send_event(TestEvent::Wake);
        }
    }));
    rx
}

pub(crate) fn hide_window_to_tray(window: &Window) {
    window.set_visible(false);
}

pub(crate) fn show_window_from_tray(window: &Window) {
    window.set_visible(true);
    window.set_minimized(false);
    window.focus_window();
}

pub(crate) fn apply_host_action(action: TrayHostAction, window: &Window) {
    match action {
        TrayHostAction::ShowMainWindow => show_window_from_tray(window),
        TrayHostAction::HideMainWindow => hide_window_to_tray(window),
        TrayHostAction::ToggleMainWindow => {
            if window.is_visible().unwrap_or(true) {
                hide_window_to_tray(window);
            } else {
                show_window_from_tray(window);
            }
        }
        TrayHostAction::QuitApp => {
            // The event loop owns actual process exit; this variant is handled by caller.
        }
    }
}

pub(crate) fn apply_activate_behavior(behavior: TrayActivateBehavior, window: &Window) {
    match behavior {
        TrayActivateBehavior::None => {}
        TrayActivateBehavior::ShowMainWindow => show_window_from_tray(window),
        TrayActivateBehavior::ToggleMainWindow => {
            apply_host_action(TrayHostAction::ToggleMainWindow, window)
        }
    }
}

fn should_activate_from_tray_event(event: &TrayIconEvent) -> bool {
    matches!(
        event,
        TrayIconEvent::Click {
            button: tray_icon::MouseButton::Left,
            button_state: tray_icon::MouseButtonState::Up,
            ..
        }
    )
}

fn build_tray_menu<S: GlobalState>(
    config: &TrayConfig<S>,
    runtime: &Runtime,
    env: &Env,
    pipeline: &Pipeline,
) -> Result<(TrayMenu, ActionRegistry<S>)> {
    let mut ctx = BuildCtx::new();
    let tray_menu = if let Some(menu_widget) = &config.menu {
        let state = runtime
            .get_global_state::<S>()
            .context("tray menu requested before app state was available")?;
        let view = View::new(
            state,
            &runtime.runtime_state,
            env,
            pipeline.last_snapshot.as_ref(),
        );
        fission_core::build::enter(&mut ctx, &view, || {
            let (ctx, view) = fission_core::build::current::<S>();
            menu_widget.menu(ctx, view)
        })
    } else {
        TrayMenu::new()
    };
    let tray_menu = if config.include_default_quit {
        tray_menu.with_default_quit()
    } else {
        tray_menu
    };
    Ok((tray_menu, ctx.registry))
}

fn fallback_tray_menu<S: GlobalState>(config: &TrayConfig<S>) -> TrayMenu {
    let tray_menu = TrayMenu::new();
    if config.include_default_quit {
        tray_menu.with_default_quit()
    } else {
        tray_menu
    }
}

fn native_menu_from_tray_menu(
    tray_menu: &TrayMenu,
) -> Result<(Menu, HashMap<String, TrayMenuAction>)> {
    let menu = Menu::new();

    let mut actions = HashMap::new();
    for (idx, entry) in tray_menu.entries().iter().enumerate() {
        match entry {
            TrayMenuEntry::Separator => {
                menu.append(&PredefinedMenuItem::separator())?;
            }
            TrayMenuEntry::Item(item) => {
                let id = MenuId::new(format!("fission-tray-menu-{idx}"));
                let native_item = MenuItemBuilder::new()
                    .id(id.clone())
                    .text(&item.label)
                    .enabled(item.enabled)
                    .build();
                menu.append(&native_item)?;
                actions.insert(id.as_ref().to_string(), item.action.clone());
            }
        }
    }
    Ok((menu, actions))
}

fn load_icon(source: &TrayIconSource) -> Result<Icon> {
    match source {
        TrayIconSource::Rgba {
            rgba,
            width,
            height,
        } => Icon::from_rgba(rgba.clone(), *width, *height)
            .map_err(|error| anyhow::anyhow!("invalid tray icon RGBA data: {error}")),
        TrayIconSource::PngBytes(bytes) => icon_from_image(image::load_from_memory(bytes)?),
        TrayIconSource::Path(path) => icon_from_path(path),
    }
}

fn icon_from_path(path: &Path) -> Result<Icon> {
    let image = image::open(path)
        .with_context(|| format!("failed to read tray icon image {}", path.display()))?;
    icon_from_image(image)
}

fn icon_from_image(image: image::DynamicImage) -> Result<Icon> {
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    Icon::from_rgba(rgba.into_raw(), width, height)
        .map_err(|error| anyhow::anyhow!("invalid tray icon image: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use fission_core::{Action, ActionId};
    use serde::{Deserialize, Serialize};

    #[derive(Default, Debug)]
    struct TrayTestState {
        refreshes: u32,
    }

    impl GlobalState for TrayTestState {}

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct RefreshTray;

    impl Action for RefreshTray {
        fn static_id() -> ActionId {
            ActionId::from_name("fission.shell.tray.tests.RefreshTray")
        }
    }

    fn refresh_tray(
        state: &mut TrayTestState,
        _action: RefreshTray,
        _ctx: &mut fission_core::ReducerContext<'_, '_, '_, TrayTestState>,
    ) {
        state.refreshes += 1;
    }

    struct DynamicTrayMenu;

    impl TrayMenuBuilder<TrayTestState> for DynamicTrayMenu {
        fn menu(
            &self,
            ctx: BuildCtxHandle<TrayTestState>,
            view: ViewHandle<TrayTestState>,
        ) -> TrayMenu {
            let refresh = ctx.bind(RefreshTray, refresh_tray as fission_core::Handler<_, _>);
            TrayMenu::new().item(
                format!("Refresh {}", view.state().refreshes),
                TrayMenuAction::app(refresh),
            )
        }
    }

    #[test]
    fn default_quit_is_added_after_custom_items() {
        let menu = TrayMenu::new()
            .item("Open", TrayMenuAction::host(TrayHostAction::ShowMainWindow))
            .with_default_quit();
        assert_eq!(menu.entries().len(), 3);
        assert!(matches!(menu.entries()[1], TrayMenuEntry::Separator));
        let TrayMenuEntry::Item(item) = &menu.entries()[2] else {
            panic!("expected quit item");
        };
        assert_eq!(item.label, "Exit");
        assert_eq!(item.action, TrayMenuAction::host(TrayHostAction::QuitApp));
    }

    #[test]
    fn default_quit_is_the_only_entry_when_no_custom_menu_exists() {
        let menu = TrayMenu::new().with_default_quit();
        assert_eq!(menu.entries().len(), 1);
        let TrayMenuEntry::Item(item) = &menu.entries()[0] else {
            panic!("expected quit item");
        };
        assert_eq!(item.label, "Exit");
        assert_eq!(item.action, TrayMenuAction::host(TrayHostAction::QuitApp));
    }

    #[test]
    fn tray_config_builder_sets_desktop_policy_and_click_behavior() {
        let config =
            TrayConfig::<TrayTestState>::new(TrayIconSource::rgba(vec![255, 255, 255, 255], 1, 1))
                .tooltip("Fission")
                .title("Running")
                .icon_template(true)
                .close_behavior(WindowCloseBehavior::HideToTray)
                .activate_behavior(TrayActivateBehavior::ShowMainWindow)
                .menu_on_left_click(true)
                .menu_on_right_click(false)
                .include_default_quit(false);

        assert_eq!(config.tooltip.as_deref(), Some("Fission"));
        assert_eq!(config.title.as_deref(), Some("Running"));
        assert!(config.icon_is_template);
        assert_eq!(config.close_behavior, WindowCloseBehavior::HideToTray);
        assert_eq!(
            config.activate_behavior,
            TrayActivateBehavior::ShowMainWindow
        );
        assert!(config.menu_on_left_click);
        assert!(!config.menu_on_right_click);
        assert!(!config.include_default_quit);
    }

    #[test]
    fn tray_menu_preserves_host_and_app_actions() {
        let app_action = TrayMenuAction::app(RefreshTray.into());
        let menu = TrayMenu::new()
            .item("Show", TrayMenuAction::host(TrayHostAction::ShowMainWindow))
            .separator()
            .disabled_item("Refresh", app_action.clone());

        assert_eq!(menu.entries().len(), 3);
        let TrayMenuEntry::Item(show) = &menu.entries()[0] else {
            panic!("expected show item");
        };
        assert_eq!(show.label, "Show");
        assert!(show.enabled);
        assert_eq!(
            show.action,
            TrayMenuAction::host(TrayHostAction::ShowMainWindow)
        );
        assert!(matches!(menu.entries()[1], TrayMenuEntry::Separator));
        let TrayMenuEntry::Item(refresh) = &menu.entries()[2] else {
            panic!("expected refresh item");
        };
        assert_eq!(refresh.label, "Refresh");
        assert!(!refresh.enabled);
        assert_eq!(refresh.action, app_action);
    }

    #[test]
    fn tray_menu_widget_build_returns_frame_registry_for_app_actions() {
        let mut runtime = Runtime::default();
        runtime
            .add_global_state(Box::new(TrayTestState::default()))
            .unwrap();
        let config =
            TrayConfig::<TrayTestState>::new(TrayIconSource::rgba(vec![255, 255, 255, 255], 1, 1))
                .include_default_quit(false)
                .menu(DynamicTrayMenu);
        let env = Env::default();
        let pipeline = Pipeline::new();

        let (menu, registry) = build_tray_menu(&config, &runtime, &env, &pipeline).unwrap();
        assert_eq!(menu.entries().len(), 1);
        let TrayMenuEntry::Item(item) = &menu.entries()[0] else {
            panic!("expected refresh item");
        };
        assert_eq!(item.label, "Refresh 0");
        let TrayMenuAction::App(action) = item.action.clone() else {
            panic!("expected app action");
        };

        runtime.clear_reducers();
        runtime
            .dispatch(action.clone(), WidgetId::from_u128(0))
            .unwrap();
        assert_eq!(
            runtime
                .get_global_state::<TrayTestState>()
                .unwrap()
                .refreshes,
            0
        );
        runtime.absorb_registry(registry);
        runtime.dispatch(action, WidgetId::from_u128(0)).unwrap();
        assert_eq!(
            runtime
                .get_global_state::<TrayTestState>()
                .unwrap()
                .refreshes,
            1
        );
    }

    #[test]
    fn rgba_icon_source_validates_buffer_size() {
        assert!(load_icon(&TrayIconSource::rgba(vec![0, 0, 0, 255], 1, 1)).is_ok());
        assert!(load_icon(&TrayIconSource::rgba(vec![0, 0, 0], 1, 1)).is_err());
    }

    #[test]
    fn click_activation_only_uses_left_button_release() {
        let rect = tray_icon::Rect::default();
        let pos = tray_icon::dpi::PhysicalPosition::new(10.0, 20.0);
        assert!(should_activate_from_tray_event(&TrayIconEvent::Click {
            id: tray_icon::TrayIconId::new("tray"),
            position: pos,
            rect,
            button: tray_icon::MouseButton::Left,
            button_state: tray_icon::MouseButtonState::Up,
        }));
        assert!(!should_activate_from_tray_event(&TrayIconEvent::Click {
            id: tray_icon::TrayIconId::new("tray"),
            position: pos,
            rect,
            button: tray_icon::MouseButton::Right,
            button_state: tray_icon::MouseButtonState::Up,
        }));
        assert!(!should_activate_from_tray_event(&TrayIconEvent::Click {
            id: tray_icon::TrayIconId::new("tray"),
            position: pos,
            rect,
            button: tray_icon::MouseButton::Left,
            button_state: tray_icon::MouseButtonState::Down,
        }));
    }
}
