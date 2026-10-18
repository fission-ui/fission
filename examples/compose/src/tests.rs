use super::{App, AppState};
use fission::compose::identity::{BindingProbe, BindingRecord};
use fission::prelude::*;
use fission_core::{self as core, ActionInput, UpdateTextInput};

struct Harness {
    state: AppState,
    runtime: core::RuntimeState,
    env: core::Env,
}

struct Built {
    _widget: Widget,
    registry: core::ActionRegistry<AppState>,
    bindings: Vec<BindingRecord>,
}

impl Harness {
    fn new() -> Self {
        Self {
            state: AppState::default(),
            runtime: core::RuntimeState::default(),
            env: crate::i18n::create_env().unwrap(),
        }
    }

    fn build(&self) -> Built {
        let mut build = core::internal::BuildCtx::<AppState>::new();
        let view = core::View::new(&self.state, &self.runtime, &self.env, None);
        let probe = BindingProbe::default();
        let widget = core::build::enter(&mut build, &view, || {
            core::build::provide(probe.clone(), || App.into())
        });
        Built {
            _widget: widget,
            registry: build.registry,
            bindings: probe.records(),
        }
    }
}

impl Built {
    fn binding(&self, symbol: &str) -> BindingRecord {
        self.bindings
            .iter()
            .find(|record| record.symbol == symbol || record.symbol.ends_with(symbol))
            .unwrap_or_else(|| panic!("missing Compose binding `{symbol}`"))
            .clone()
    }

    fn dispatch(&mut self, state: &mut AppState, record: &BindingRecord, input: ActionInput) {
        self.registry
            .dispatch_with_input(state, &record.envelope, record.node, &input)
            .unwrap();
    }
}

fn text_input(value: &str) -> ActionInput {
    ActionInput::TextChanged(UpdateTextInput {
        new_text: value.into(),
        ..Default::default()
    })
}

#[test]
fn product_list_inline_action_creates_and_navigates_to_a_product() {
    let mut harness = Harness::new();
    harness.state.current_path = "/products".into();
    let mut built = harness.build();
    let add = built.binding("/add-product/on_press");
    let before = harness.state.products.len();

    built.dispatch(&mut harness.state, &add, ActionInput::None);

    assert_eq!(harness.state.products.len(), before + 1);
    assert!(harness.state.current_path.starts_with("/products/"));
}

#[test]
fn editor_exercises_scoped_fields_local_state_and_inline_handlers() {
    let mut harness = Harness::new();
    harness.state.current_path = "/products/101".into();
    let mut built = harness.build();
    let name = built.binding("state.name");
    let active = built.binding("state.active");
    let notes_toggle = built.binding("local.show_notes");

    built.dispatch(&mut harness.state, &name, text_input("Standing desk"));
    built.dispatch(&mut harness.state, &active, ActionInput::None);
    built.dispatch(&mut harness.state, &notes_toggle, ActionInput::None);

    let mut rebuilt = harness.build();
    let notes = rebuilt.binding("state.notes");
    let save = rebuilt.binding("/save/on_press");
    rebuilt.dispatch(&mut harness.state, &notes, text_input("Ships flat"));
    rebuilt.dispatch(&mut harness.state, &save, ActionInput::None);

    let product = &harness.state.products[&101];
    assert_eq!(product.name, "Standing desk");
    assert_eq!(product.notes, "Ships flat");
    assert!(
        product.active,
        "save reactivates a product when notes are shown"
    );
    assert_eq!(product.saves, 1);
}

#[test]
fn removed_control_rejects_its_old_binding() {
    let mut harness = Harness::new();
    harness.state.current_path = "/products/101".into();
    let mut closed = harness.build();
    let notes_toggle = closed.binding("local.show_notes");
    closed.dispatch(&mut harness.state, &notes_toggle, ActionInput::None);

    let mut open = harness.build();
    let stale_notes = open.binding("state.notes");
    let notes_toggle = open.binding("local.show_notes");
    open.dispatch(&mut harness.state, &notes_toggle, ActionInput::None);
    let _closed_again = harness.build();

    open.dispatch(&mut harness.state, &stale_notes, text_input("late event"));
    assert!(harness.state.products[&101].notes.is_empty());
}

#[test]
fn settings_bindings_are_isolated_from_product_state() {
    let mut harness = Harness::new();
    harness.state.current_path = "/settings".into();
    let mut built = harness.build();
    let notifications = built.binding("state.settings.notifications");
    built.dispatch(&mut harness.state, &notifications, ActionInput::None);

    assert!(!harness.state.settings.notifications);
    assert_eq!(harness.state.products.len(), 3);
}

#[test]
fn every_route_builds_through_the_native_router() {
    for path in ["/", "/products", "/products/101", "/settings", "/missing"] {
        let mut harness = Harness::new();
        harness.state.current_path = path.into();
        let _built = harness.build();
    }
}

#[test]
fn changing_language_updates_state_and_selects_the_spanish_bundle() {
    let mut harness = Harness::new();
    harness.state.current_path = "/settings".into();
    let mut built = harness.build();
    let spanish = built.binding("/spanish/on_press");

    built.dispatch(&mut harness.state, &spanish, ActionInput::None);
    harness.env.locale = harness.state.locale.clone();

    assert_eq!(harness.state.locale, fission::i18n::Locale::from("es-ES"));
    assert_eq!(harness.env.tr("app.title"), "Inventario de Fission Compose");
    assert_eq!(harness.env.tr("settings.title"), "Ajustes");
    assert_eq!(harness.env.tr("editor.save"), "Guardar producto");
}
