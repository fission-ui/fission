//! Declarative component and state-scope macros; application control flow
//! remains ordinary Rust.
use proc_macro::TokenStream;
use proc_macro2::TokenStream as Tokens;
use proc_macro_crate::{crate_name, FoundCrate};
use quote::{format_ident, quote};
use syn::parse_macro_input;
mod ast;
mod effect;
mod expand;
mod scope;

fn support() -> Tokens {
    match crate_name("fission-compose") {
        Ok(FoundCrate::Itself) => quote!(::fission_compose),
        Ok(FoundCrate::Name(name)) => {
            let id = format_ident!("{}", name);
            quote!(::#id)
        }
        Err(_) => match crate_name("fission") {
            Ok(FoundCrate::Itself) => quote!(::fission::compose),
            Ok(FoundCrate::Name(name)) => {
                let id = format_ident!("{}", name);
                quote!(::#id::compose)
            }
            Err(_) => quote!(::fission_compose),
        },
    }
}
#[proc_macro]
pub fn fission_component(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ast::Component);
    expand::component(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro_attribute]
pub fn fission_effect(_args: TokenStream, input: TokenStream) -> TokenStream {
    let function = parse_macro_input!(input as syn::ItemFn);
    effect::expand(function)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
#[proc_macro]
pub fn state_scope(input: TokenStream) -> TokenStream {
    scope::expand(input.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    fn expand(input: Tokens) -> syn::Result<Tokens> {
        expand::component(syn::parse2(input)?)
    }
    fn rejects(input: Tokens, message: &str) {
        let error = match expand(input) {
            Ok(_) => panic!("expected rejection"),
            Err(e) => e,
        };
        assert!(error.to_string().contains(message), "{error}");
    }
    #[test]
    fn generated_items_are_valid_rust_syntax() {
        let output = expand(quote! {
            pub Editor<State> {
                props { session: u64 }
                state { open: bool = false }
            }
            view(editor, state, cx) => Column(children: [
                TextInput(label: Some("Name".into()), bind: state.name),
                Switch(checked: editor.open, bind(on_toggle): to(editor.open)),
                Button(child: Text("Save"), key: "save", on_press: reduce {
                    state.name = format!("{}", editor.session);
                    editor.open = !editor.open;
                }),
            ])
        })
        .unwrap();
        syn::parse2::<syn::File>(output).unwrap();
    }

    #[test]
    fn effect_functions_and_component_lifecycle_blocks_expand_to_rust() {
        let effect = effect::expand(
            syn::parse2(quote! {
                pub async fn increment_counter(current: i32) -> Result<i32, String> {
                    Ok(current + 1)
                }
            })
            .unwrap(),
        )
        .unwrap();
        syn::parse2::<syn::File>(effect).unwrap();

        let output = expand(quote! {
            pub RemoteCounter<()> {
                state {
                    count: i32 = 0,
                    loading: bool = false,
                    error: Option<String> = None,
                }
            }
            view(counter, _state, _cx) => Button(
                key: "increment",
                child: Text("Increment via API"),
                on_press: effect increment_counter(counter.count) {
                    on_start {
                        counter.loading = true;
                        counter.error = None;
                    }
                    on_ok(count) {
                        counter.count = count;
                        counter.loading = false;
                    }
                    on_err(error) {
                        counter.error = Some(error);
                        counter.loading = false;
                    }
                },
            )
        })
        .unwrap();
        syn::parse2::<syn::File>(output.clone()).unwrap();
        let output = output.to_string();
        assert!(output.contains("app_effect"), "{output}");
        assert!(output.contains("job_ok"), "{output}");
        assert!(output.contains("job_err"), "{output}");
    }

    #[test]
    fn effect_function_contract_rejects_unsupported_signatures() {
        let rejects_effect = |tokens: Tokens, message: &str| {
            let function = syn::parse2(tokens).expect("effect function syntax");
            let error = effect::expand(function).expect_err("expected effect rejection");
            assert!(error.to_string().contains(message), "{error}");
        };

        rejects_effect(
            quote!(
                fn load() -> Result<(), String> {
                    Ok(())
                }
            ),
            "requires an async function",
        );
        rejects_effect(
            quote!(
                async fn load<T>(value: T) -> Result<T, String> {
                    Ok(value)
                }
            ),
            "cannot be generic",
        );
        rejects_effect(
            quote!(
                async fn load(&self) -> Result<(), String> {
                    Ok(())
                }
            ),
            "cannot have a receiver",
        );
        rejects_effect(
            quote!(
                async fn load(value: String) {
                    drop(value);
                }
            ),
            "must return Result",
        );
        rejects_effect(
            quote!(
                async fn load((left, right): (i32, i32)) -> Result<i32, String> {
                    Ok(left + right)
                }
            ),
            "simple identifier patterns",
        );
    }

    #[test]
    fn effect_on_start_is_optional_but_both_completion_paths_are_required() {
        let output = expand(quote! {
            pub Counter<()> { state { count: i32 = 0, error: Option<String> = None } }
            view(counter, _state, _cx) => Button(
                key: "increment",
                on_press: effect increment_counter(counter.count) {
                    on_ok(count) { counter.count = count; }
                    on_err(error) { counter.error = Some(error); }
                },
            )
        })
        .unwrap();
        syn::parse2::<syn::File>(output).unwrap();

        rejects(
            quote! {
                pub Counter<()> { state { count: i32 = 0 } }
                view(counter, _state, _cx) => Button(
                    key: "increment",
                    on_press: effect increment_counter(counter.count) {
                        on_err(_error) {}
                    },
                )
            },
            "effect requires on_ok",
        );
        rejects(
            quote! {
                pub Counter<()> { state { count: i32 = 0 } }
                view(counter, _state, _cx) => Button(
                    key: "increment",
                    on_press: effect increment_counter(counter.count) {
                        on_ok(count) { counter.count = count; }
                    },
                )
            },
            "effect requires on_err",
        );
    }

    #[test]
    fn effect_arguments_reject_live_bindings_and_borrows() {
        let component = |argument: Tokens| {
            quote! {
                pub Counter<AppState> {
                    state { count: i32 = 0, error: Option<String> = None }
                }
                view(counter, state, cx) => Button(
                    key: "increment",
                    on_press: effect increment_counter(#argument) {
                        on_ok(count) { counter.count = count; }
                        on_err(error) { counter.error = Some(error); }
                    },
                )
            }
        };
        rejects(
            component(quote!(counter)),
            "cannot receive the component binding",
        );
        rejects(
            component(quote!(state)),
            "cannot receive the global-state binding",
        );
        rejects(component(quote!(cx)), "cannot receive the reducer context");
        rejects(
            component(quote!(&counter.count)),
            "effect arguments must be owned",
        );
        rejects(
            component(quote!(&state.session)),
            "effect arguments must be owned",
        );
    }

    #[test]
    fn action_envelope_slots_accept_inline_handlers_without_a_widget_allowlist() {
        let output = expand(quote! {
            pub Dialog<State> |dialog, state, cx| => Modal(
                key: "dialog",
                on_dismiss: |_dialog, state, _cx| state.open = false,
            )
        })
        .unwrap();

        syn::parse2::<syn::File>(output.clone()).unwrap();
        let output = output.to_string();
        assert!(output.contains("events :: Press"), "{output}");
        assert!(output.contains("__widget . on_dismiss = Some"), "{output}");
    }

    #[test]
    fn state_agnostic_components_do_not_request_unit_view_state() {
        let output = expand(quote! {
            pub Caption<()> { text: String } |caption, _state, _cx| => Text(caption.text.clone())
        })
        .unwrap();

        syn::parse2::<syn::File>(output.clone()).unwrap();
        let output = output.to_string();
        assert!(output.contains("let _state : & () = & ()"), "{output}");
        assert!(!output.contains("__view . state ()"), "{output}");
    }
    #[test]
    fn component_attributes_are_preserved_without_duplicate_clone_derives() {
        let output = expand(quote! {
            #[derive(Clone, Debug)]
            /// A documented component.
            pub Screen<State> |screen, state, cx| => Text("Ready")
        })
        .unwrap()
        .to_string();

        assert!(output.contains("derive (Clone , Debug)"), "{output}");
        assert_eq!(output.matches("derive (Clone").count(), 1, "{output}");
        assert!(output.contains("A documented component"), "{output}");
    }
    #[test]
    fn named_arguments_lower_to_struct_fields_without_generated_builders() {
        let output = expand(quote! {
            pub Screen<State> |screen, state, cx| =>
                ThirdPartyWidget(title: "Imported", enabled: true)
        })
        .unwrap()
        .to_string();

        assert!(output.contains("__widget : ThirdPartyWidget"), "{output}");
        assert!(output.contains("__widget . title ="), "{output}");
        assert!(output.contains("__widget . enabled ="), "{output}");
        assert!(output.contains("Default :: default"), "{output}");
        assert!(!output.contains(". title ("), "{output}");
        assert!(!output.contains(". enabled ("), "{output}");
    }
    #[test]
    fn authored_widget_ids_are_not_overwritten_by_compose_identity() {
        let output = expand(quote! {
            pub Screen<State> |screen, state, cx| => TextInput(
                id: Some(WidgetId::explicit("name")),
                bind: state.name,
            )
        })
        .unwrap()
        .to_string();

        assert!(output.contains("__widget . id ="), "{output}");
        assert!(!output.contains("WidgetIdExt :: id"), "{output}");
    }

    #[test]
    fn static_children_receive_shape_slots_and_identical_siblings_are_disambiguated() {
        let output = expand(quote! {
            pub Screen<State> |screen, state, cx| => Column(children: [
                if state.first { Text("First") } else { Spacer() },
                if state.second { Text("Second") } else { Spacer() },
                Spacer(),
                Spacer(),
            ])
        })
        .unwrap()
        .to_string();

        assert_eq!(output.matches("duplicate:0").count(), 1, "{output}");
        assert_eq!(output.matches("duplicate:1").count(), 1, "{output}");
        assert_eq!(
            output.matches("identity :: in_slot (\"child:").count(),
            4,
            "{output}"
        );
    }
    #[test]
    fn generated_components_protect_their_owned_tree_with_a_composite_boundary() {
        let output = expand(quote! {
            pub Screen<State> |screen, state, cx| => ThirdPartyWidget(
                title: "Imported",
            )
        })
        .unwrap()
        .to_string();

        assert!(output.contains("core :: ui :: Composite"), "{output}");
        assert!(output.contains("id : Some (__owner)"), "{output}");
        assert!(output.contains("WidgetIdExt :: id"), "{output}");
    }
    #[test]
    fn non_child_arrays_remain_rust_arrays() {
        let output = expand(quote! {
            pub Screen<State> |screen, state, cx| => Button(
                padding: [1.0, 2.0, 3.0, 4.0],
                child: Text("Save"),
            )
        })
        .unwrap()
        .to_string();

        assert!(output.contains("__widget . padding ="), "{output}");
        assert!(output.contains("as f32"), "{output}");
        assert!(!output.contains("Vec <"), "{output}");
    }
    #[test]
    fn setup_statements_and_explicit_widget_modifiers_remain_ordinary_rust() {
        let output = expand(quote! {
            pub Screen<State> |screen, state, cx| {
                let padding = cx.view.env().theme.tokens.spacing.l;
            } => Container(
                Column(children: [Text("Ready")]),
            )
            .padding_all(padding)
            .width_length(Length::percent(100.0))
        })
        .unwrap();
        syn::parse2::<syn::File>(output.clone()).unwrap();
        let output = output.to_string();

        assert!(output.contains("let padding ="), "{output}");
        assert!(output.contains(". padding_all (padding)"), "{output}");
        assert!(output.contains(". width_length"), "{output}");
    }

    #[test]
    fn components_default_to_their_root_state_without_scope_boilerplate() {
        let expanded = expand(quote! {
            pub App<AppState> |app, state, cx| => Text(state.title.clone())
        })
        .unwrap()
        .to_string();

        assert!(expanded.contains("struct __AppRootScope"));
        assert!(expanded.contains("type Root = AppState"));
        assert!(expanded.contains("type Target = AppState"));
        assert!(!expanded.contains("DefaultScope"));
    }
    #[test]
    fn rust_prelude_tuple_constructors_remain_rust_expressions() {
        let output = expand(quote! {
            pub Screen<State> |screen, state, cx| => ThirdPartyWidget(
                optional: Some("value".to_owned()),
                success: Ok(3),
                failure: Err("no"),
            )
        })
        .unwrap()
        .to_string();

        assert!(
            output.contains("Some (\"value\" . to_owned ())"),
            "{output}"
        );
        assert!(output.contains("Ok (3)"), "{output}");
        assert!(output.contains("Err (\"no\")"), "{output}");
        assert!(!output.contains("Some {"), "{output}");
        assert!(!output.contains("Ok {"), "{output}");
        assert!(!output.contains("Err {"), "{output}");
    }
    #[test]
    fn some_can_wrap_a_composed_widget() {
        let output = expand(quote! {
            pub Screen<State> |screen, state, cx| => EmptyState(
                icon: Some(Icon(name: "inbox")),
            )
        })
        .unwrap()
        .to_string();

        assert!(output.contains("__widget . icon = (Some"), "{output}");
        assert!(output.contains("__widget : Icon"), "{output}");
    }

    #[test]
    fn expressions_beginning_with_some_remain_rust_expressions() {
        let output = expand(quote! {
            pub Screen<State> |screen, state, cx| => ProductCard(
                selected: Some(screen.id) == state.selected_id,
            )
        })
        .unwrap()
        .to_string();

        assert!(
            output.contains("Some (screen . id) == state . selected_id"),
            "{output}"
        );
        assert!(!output.contains("Some (Some"), "{output}");
    }

    #[test]
    fn method_calls_on_if_expressions_remain_rust_expressions() {
        let output = expand(quote! {
            pub Screen<State> |screen, state, cx| => Label(
                if state.ready { "Ready" } else { "Waiting" }.to_string(),
            )
        })
        .unwrap()
        .to_string();

        assert!(output.contains(". to_string ()"), "{output}");
    }

    #[test]
    fn qualified_enum_constructors_remain_rust_expressions() {
        let output = expand(quote! {
            pub Screen<State> |screen, state, cx| =>
                Text(TextContent::Key("screen.title".into()))
        })
        .unwrap()
        .to_string();

        assert!(output.contains("TextContent :: Key"), "{output}");
        assert!(!output.contains("TextContent :: Key {"), "{output}");
    }

    #[test]
    fn positional_imported_widgets_receive_a_distinct_stable_scope() {
        let first = expand(quote! {
            pub Screen<State> |screen, state, cx| =>
                Container(ThirdPartyWidget { title: "Imported".into() })
        })
        .unwrap()
        .to_string();
        let second = expand(quote! {
            pub Screen<State> |screen, state, cx| =>
                Container(ThirdPartyWidget { title: "Imported".into() })
        })
        .unwrap()
        .to_string();

        assert!(first.contains("in_slot (\"argument:"), "{first}");
        assert_eq!(first, second, "positional scopes must be deterministic");
    }

    #[test]
    fn component_fields_are_cloned_when_read_as_children() {
        let output = expand(quote! {
            pub Layout<State> { content: Widget } |layout, state, cx| =>
                Column(children: [layout.content])
        })
        .unwrap()
        .to_string();

        assert!(
            output.contains("Clone :: clone (& (layout . content))"),
            "{output}"
        );
    }

    #[test]
    fn conditional_component_branches_can_have_different_concrete_types() {
        let output = expand(quote! {
            pub Screen<State> |screen, state, cx| => if state.expanded {
                ExpandedView
            } else {
                CompactView
            }
        })
        .unwrap();
        let parsed = syn::parse2::<syn::File>(output.clone());
        assert!(parsed.is_ok(), "{}", output);
        assert!(output.to_string().contains("let __branch"));
    }
    #[test]
    fn conditional_property_values_remain_ordinary_rust() {
        let output = expand(quote! {
            pub Screen<State> |screen, state, cx| => Button(
                child: Text(if state.ready { "Ready".to_owned() } else { "Waiting".to_owned() }),
                variant: if state.ready {
                    ButtonVariant::Primary
                } else {
                    ButtonVariant::Ghost
                },
            )
        })
        .unwrap();
        syn::parse2::<syn::File>(output.clone()).unwrap();
        assert!(!output.to_string().contains("let __branch"));
    }
    #[test]
    fn duplicate_fields_are_an_error() {
        rejects(
            quote! { X<S> { x: bool, x: bool } |e,s,c| => Text("x") },
            "duplicate component field",
        );
    }
    #[test]
    fn duplicate_props_are_an_error() {
        rejects(
            quote! { X<S> |e,s,c| => Text("x", size: 12.0, size: 14.0) },
            "duplicate",
        );
    }
    #[test]
    fn repeated_event_bindings_target_distinct_widget_properties() {
        let output = expand(quote! {
            pub RangeEditor<State> |editor,state,cx| => RangeControl(
                lower: state.lower,
                bind(on_lower_change as LowerChanged): to(state.lower),
                upper: state.upper,
                bind(on_upper_change as UpperChanged): to(state.upper),
            )
        })
        .unwrap();
        syn::parse2::<syn::File>(output.clone()).unwrap();
        let output = output.to_string();

        assert!(output.contains("__widget . on_lower_change ="), "{output}");
        assert!(output.contains("__widget . on_upper_change ="), "{output}");
        assert!(output.contains("LowerChanged"), "{output}");
        assert!(output.contains("UpperChanged"), "{output}");
    }
    #[test]
    fn duplicate_event_binding_is_an_error() {
        rejects(
            quote! {
                X<S> |e,s,c| => TextInput(
                    bind(on_input): to(s.first),
                    bind(on_input): to(s.second),
                )
            },
            "duplicate binding for event property",
        );
    }
    #[test]
    fn unkeyed_dynamic_children_are_an_error() {
        rejects(
            quote! { X<S> |e,s,c| => Column(children:[ for item in &s.items { Text(item.name.clone()) } ]) },
            "explicit key",
        );
    }
    #[test]
    fn unkeyed_static_inline_handlers_use_the_authored_shape() {
        let output = expand(
            quote! { X<S> |e,s,c| => Button(child: Text("Save"), on_press: |_,s,_| s.save()) },
        )
        .unwrap();
        syn::parse2::<syn::File>(output).unwrap();
    }
    #[test]
    fn dynamically_keyed_inline_handlers_need_a_semantic_name() {
        rejects(
            quote! { X<S> { id:String } |e,s,c| => Button(child: Text("Save"), key:e.id.clone(), on_press: |_,s,_| s.save()) },
            "with a dynamic control key",
        );
    }
    #[test]
    fn reducers_can_capture_typed_dynamic_payloads() {
        let output = expand(quote! {
            Picker<()> { state { selected: Option<u64> = None } }
            view(picker, _state, _cx) => Column(children: [
                for id in [7_u64, 9_u64] {
                    Button(
                        child: Text(id.to_string()),
                        key: id,
                        on_press: reduce select(selected_id: u64 = id) {
                            picker.selected = Some(selected_id);
                        },
                    )
                },
            ])
        })
        .unwrap();
        syn::parse2::<syn::File>(output.clone()).unwrap();
        let output = output.to_string();
        assert!(output.contains("selected_id : u64"), "{output}");
        assert!(output.contains("selected_id : id"), "{output}");
    }
    #[test]
    fn named_reducers_replace_separate_dynamic_handler_names() {
        rejects(
            quote! {
                Picker<()> view(picker, _state, _cx) => Button(
                    key: picker.id,
                    handler: "select",
                    on_press: reduce select(id: u64 = picker.id) {},
                )
            },
            "named reducer already supplies its handler identity",
        );
    }
    #[test]
    fn reducer_capture_names_are_unique_and_cannot_shadow_view_bindings() {
        rejects(
            quote! {
                Picker<()> view(picker, state, cx) => Button(
                    on_press: reduce(id: u64 = 1, id: u64 = 2) {},
                )
            },
            "duplicate reducer capture",
        );
        rejects(
            quote! {
                Picker<()> view(picker, state, cx) => Button(
                    on_press: reduce(state: u64 = 1) {},
                )
            },
            "cannot shadow",
        );
        rejects(
            quote! {
                Picker<()> view(picker, state, cx) => Button(
                    on_press: reduce(props: u64 = 1) {},
                )
            },
            "reserved reducer capture names",
        );
    }
    #[test]
    fn handler_capture_and_async_are_rejected() {
        rejects(
            quote! { X<S> |e,s,c| => Button(child: Text("Save"), key:"save", on_press: move |_,s,_| s.save()) },
            "no move",
        );
        rejects(
            quote! { X<S> |e,s,c| => Button(child: Text("Save"), key:"save", on_press: |_,s,_| s.save().await) },
            "synchronous",
        );
    }
    #[test]
    fn local_declaration_reorder_keeps_handler_symbols() {
        fn events(tokens: Tokens) -> Vec<String> {
            let f: syn::File = syn::parse2(expand(tokens).unwrap()).unwrap();
            f.items
                .into_iter()
                .filter_map(|item| match item {
                    syn::Item::Struct(s) if s.ident.to_string().contains("_Event_") => {
                        Some(s.ident.to_string())
                    }
                    _ => None,
                })
                .collect()
        }
        let a = events(
            quote! { X<S> { #[local_state(default=false)] a:bool, #[local_state(default=false)] b:bool }
            |e,s,c| => Button(child:Text("Save"),key:"save",on_press:|_,s,_|s.save()) },
        );
        let b = events(
            quote! { X<S> { #[local_state(default=false)] b:bool, #[local_state(default=false)] a:bool }
            |e,s,c| => Button(child:Text("Save"),key:"save",on_press:|_,s,_|s.save()) },
        );
        assert_eq!(a, b);
        assert_eq!(a.len(), 1);
    }
    #[test]
    fn scope_and_props_expansions_are_rust_items() {
        for input in [
            quote!(pub Root => Root),
            quote!(pub Root.products => Products),
            quote!(pub Editors(Root,u64=>Product){get:|s,k|s.rows.get(k),get_mut:|s,k|s.rows.get_mut(k)}),
        ] {
            syn::parse2::<syn::File>(scope::expand(input).unwrap()).unwrap();
        }
    }
}
