use fission::prelude::GlobalState;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DocsState;

impl GlobalState for DocsState {}
