use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Tooltip {
    pub trigger: String, // "item", "axis"
}

impl Tooltip {
    pub fn axis_trigger() -> Self {
        Self { trigger: "axis".into() }
    }
    
    pub fn item_trigger() -> Self {
        Self { trigger: "item".into() }
    }
}
