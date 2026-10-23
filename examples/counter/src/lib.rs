use fission::prelude::*;

fission_component! {
    pub CounterApp<()> {
        state { count: i32 = 0 }
    }
    view(counter, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    Container(
        Column(
            gap: tokens.spacing.l,
            children: [
                Text("Counter")
                    .size(tokens.typography.heading1_size)
                    .color(tokens.colors.text_primary),
                Text(counter.count.to_string())
                    .size(tokens.typography.display_md_size)
                    .color(tokens.colors.primary),
                Row(
                    gap: tokens.spacing.m,
                    children: [
                        // One primary action per view: incrementing is the main
                        // action, so decrementing uses the outline hierarchy.
                        Button(
                            child: Text("Decrement"),
                            key: "decrement",
                            variant: ButtonVariant::Outline,
                            on_press: reduce { counter.count -= 1; },
                        )
                        .semantics_identifier("counter.decrement"),
                        Button(
                            child: Text("Increment"),
                            key: "increment",
                            on_press: reduce { counter.count += 1; },
                        )
                        .semantics_identifier("counter.increment"),
                    ],
                ),
            ],
        ),
    )
    .padding_all(tokens.spacing.xl)
    // Fill the window so the background reaches every edge, not just the content.
    .width_length(Length::percent(100.0))
    .height_length(Length::percent(100.0))
    .bg(tokens.colors.background)
}
