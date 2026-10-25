use fission_core::authoring::BuildCtx;
use fission_core::ui::{Slider, Widget};
use fission_core::{build, ActionEnvelope, ActionId, Env, RuntimeState, View};
use fission_ir::{semantics::ActionTrigger, CoreIR, Op, Role};
use fission_shell_site::{render_ir_to_html, HtmlRenderOptions};
use fission_widgets::{
    FormControlLayout, FormDescription, FormError, FormLabel, NumberInput, SegmentedControl,
};
use std::sync::Arc;

fn lower_component(component: impl FnOnce() -> Widget) -> CoreIR {
    let env = Env::default();
    let runtime = RuntimeState::default();
    let state = ();
    let view = View::new(&state, &runtime, &env, None);
    let mut build_ctx = BuildCtx::<()>::new();
    let widget = build::enter(&mut build_ctx, &view, component);
    let mut lowering = fission_core::internal::LoweringContext::new(&env, &runtime, None, None);
    let root = fission_core::internal::lower_widget(&widget, &mut lowering);
    lowering.set_root(root);
    lowering.ir().clone()
}

fn assert_single_native_slider(ir: &CoreIR, html: &str) {
    assert_eq!(html.matches("type=\"range\"").count(), 1);
    let sliders: Vec<_> = ir
        .nodes
        .values()
        .filter(|node| matches!(&node.op, Op::Semantics(s) if s.role == Role::Slider))
        .collect();
    assert_eq!(sliders.len(), 1);
    let slider = sliders[0];
    assert!(
        !slider.children.is_empty(),
        "retained slider has a visual subtree"
    );
    for child in &slider.children {
        assert!(
            !html.contains(&format!("data-fission-node=\"{child}\"")),
            "native range must replace the retained track/thumb subtree"
        );
    }
    assert!(html.contains(&format!("data-fission-node=\"{}\"", slider.id)));
}

#[test]
fn native_slider_replaces_painted_track_and_retains_bounds_value_and_change_target() {
    let action = ActionEnvelope {
        id: ActionId::from_name("native-slider-test::volume-changed"),
        payload: br#"{"value":25.5}"#.to_vec(),
    };
    let ir = lower_component(|| {
        Slider {
            min: -10.0,
            max: 100.0,
            value: 25.5,
            on_change: Some(action.clone()),
            ..Default::default()
        }
        .semantics_identifier("volume-slider")
        .into()
    });
    let rendered = render_ir_to_html(
        &ir,
        &HtmlRenderOptions {
            browser_action_bindings: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_single_native_slider(&ir, &rendered.html);
    let input = rendered
        .html
        .split("<input ")
        .nth(1)
        .unwrap()
        .split('>')
        .next()
        .unwrap();
    for attribute in [
        "class=\"fission-site-range\"",
        "min=\"-10\"",
        "max=\"100\"",
        "value=\"25.500\"",
        "step=\"any\"",
        "aria-valuemin=\"-10\"",
        "aria-valuemax=\"100\"",
        "aria-valuenow=\"25.5\"",
        "tabindex=\"0\"",
        "data-fission-semantics=\"volume-slider\"",
    ] {
        assert!(input.contains(attribute), "missing {attribute}");
    }
    let semantics = ir
        .nodes
        .values()
        .find_map(|node| match &node.op {
            Op::Semantics(s) if s.role == Role::Slider => Some(s),
            _ => None,
        })
        .unwrap();
    assert_eq!(semantics.actions.entries.len(), 1);
    assert_eq!(semantics.actions.entries[0].trigger, ActionTrigger::Change);
    assert_eq!(semantics.actions.entries[0].action_id, action.id.as_u128());
    assert_eq!(
        semantics.actions.entries[0].payload_data.as_deref(),
        Some(action.payload.as_slice())
    );
}

#[test]
fn native_slider_emits_an_explicit_discrete_step() {
    let ir = lower_component(|| {
        Slider {
            min: 1.0,
            max: 250.0,
            value: 25.4,
            step: Some(1.0),
            ..Default::default()
        }
        .into()
    });
    let rendered = render_ir_to_html(&ir, &HtmlRenderOptions::default()).unwrap();

    assert_single_native_slider(&ir, &rendered.html);
    assert!(rendered.html.contains("step=\"1\""));
    assert!(rendered.html.contains("value=\"25\""));
    assert!(!rendered.html.contains("step=\"any\""));
}

#[test]
fn native_slider_keeps_its_surrounding_form_label_help_and_error() {
    let ir = lower_component(|| {
        FormControlLayout::new(
            fission_ir::WidgetId::explicit("brightness-field"),
            Slider {
                min: 0.0,
                max: 10.0,
                value: 4.0,
                ..Default::default()
            },
        )
        .label(FormLabel::new("Brightness"))
        .description(FormDescription::new("Choose the display brightness"))
        .error(FormError::new("Choose a lower brightness"))
        .into()
    });
    let rendered = render_ir_to_html(&ir, &HtmlRenderOptions::default()).unwrap();
    assert_single_native_slider(&ir, &rendered.html);
    for message in [
        "Brightness",
        "Choose the display brightness",
        "Choose a lower brightness",
    ] {
        assert!(rendered.html.contains(message), "missing {message}");
    }
}

#[test]
fn number_input_retains_one_editable_field_and_both_step_buttons() {
    let ir = lower_component(|| {
        NumberInput {
            value: 3.0,
            field_width: Some(52.0),
            min: Some(1.0),
            max: Some(20.0),
            label: Some("Copies".into()),
            on_decrement: Some(ActionEnvelope {
                id: ActionId::from_name("native-number-input-test::decrement"),
                payload: Vec::new(),
            }),
            on_increment: Some(ActionEnvelope {
                id: ActionId::from_name("native-number-input-test::increment"),
                payload: Vec::new(),
            }),
            ..Default::default()
        }
        .into()
    });
    let rendered = render_ir_to_html(
        &ir,
        &HtmlRenderOptions {
            browser_action_bindings: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(rendered.html.matches("<input ").count(), 1);
    assert_eq!(rendered.html.matches("<button ").count(), 2);
    let button_openings = rendered
        .html
        .split("<button ")
        .skip(1)
        .map(|button| button.split('>').next().unwrap())
        .collect::<Vec<_>>();
    assert!(button_openings[0].contains("aria-label=\"Decrease\""));
    assert!(button_openings[1].contains("aria-label=\"Increase\""));
    assert!(rendered.html.contains("role=\"spinbutton\""));
    assert!(rendered
        .html
        .contains("class=\"fission-site-node fission-site-control\" style=\"width:52px\""));
    for attribute in [
        "aria-label=\"Copies\"",
        "aria-label=\"Decrease\"",
        "aria-label=\"Increase\"",
        "aria-valuemin=\"1\"",
        "aria-valuemax=\"20\"",
        "aria-valuenow=\"3\"",
    ] {
        assert!(rendered.html.contains(attribute), "missing {attribute}");
    }
}

#[test]
fn segmented_control_renders_bound_native_radios_without_duplicate_labels() {
    let ir = lower_component(|| {
        SegmentedControl {
            options: vec!["Monthly".into(), "Yearly".into()],
            selected_index: 1,
            on_change: Some(Arc::new(|index| ActionEnvelope {
                id: ActionId::from_name("native-segmented-control-test::change"),
                payload: vec![index as u8],
            })),
        }
        .into()
    });
    let rendered = render_ir_to_html(
        &ir,
        &HtmlRenderOptions {
            browser_action_bindings: true,
            ..Default::default()
        },
    )
    .unwrap();

    assert_eq!(rendered.html.matches("type=\"radio\"").count(), 2);
    assert_eq!(
        rendered
            .html
            .matches("data-fission-browser-action=\"true\"")
            .count(),
        2
    );
    assert_eq!(rendered.html.matches(" checked").count(), 1);
    assert!(!rendered
        .html
        .contains("fission-site-control-label\">Monthly"));
    assert!(!rendered
        .html
        .contains("fission-site-control-label\">Yearly"));
}
