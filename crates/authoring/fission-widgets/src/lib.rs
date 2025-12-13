#![allow(unused_imports)]

use fission_macros::Action;
use fission_core::{Action as CoreAction, ActionId};
use serde::{Serialize, Deserialize};

#[derive(Action, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct MyTestAppAction { pub value: u32 }

// We will add widgets here later
