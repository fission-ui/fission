use fission::prelude::*;

pub(crate) trait ShowcaseSemantics {
    fn link(label: impl Into<String>) -> Self;
    fn identifier(self, identifier: impl Into<String>) -> Self;
}

impl ShowcaseSemantics for Semantics {
    fn link(label: impl Into<String>) -> Self {
        Self {
            role: Role::Link,
            label: Some(label.into()),
            focusable: true,
            ..Default::default()
        }
    }

    fn identifier(mut self, identifier: impl Into<String>) -> Self {
        self.identifier = Some(identifier.into());
        self
    }
}
