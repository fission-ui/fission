//! The check that a design system declares every recipe widgets read.

use super::*;

impl Package {
    /// Fails the build when a design system omits a component recipe that the
    /// generated theme resolves.
    ///
    /// Without this, a missing recipe falls back to geometry inlined in this
    /// crate. Colour, radius and typography still resolve through the design
    /// system's own tokens, so the result is not visibly foreign and nobody
    /// notices; but the control sizes, line heights and insets are then this
    /// crate's rather than the design system's, and the design system has no
    /// way to say otherwise because there is no recipe for it to say it in.
    ///
    /// Widgets should look like the design system that is loaded. If a recipe
    /// is genuinely optional, read it with an explicit fallback and drop it
    /// from this list.
    pub(crate) fn check_required_components(&self) -> Result<()> {
        let present = self
            .dsp
            .pointer("/components")
            .and_then(Value::as_object)
            .ok_or_else(|| anyhow!("{} has no components object", self.dsp_path.display()))?;
        let missing = REQUIRED_COMPONENT_RECIPES
            .iter()
            .filter(|name| !present.contains_key(**name))
            .copied()
            .collect::<Vec<_>>();
        if missing.is_empty() {
            return Ok(());
        }
        Err(anyhow!(
            "{} is missing component {}: {}.\n\
             The generated theme resolves {} for every design system, so an \
             omitted one silently falls back to fission-design-system-codegen's \
             own geometry instead of this design system's.",
            self.dsp_path.display(),
            if missing.len() == 1 {
                "recipe"
            } else {
                "recipes"
            },
            missing.join(", "),
            if missing.len() == 1 { "it" } else { "them" },
        ))
    }
}
