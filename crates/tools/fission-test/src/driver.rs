use crate::TestHarness;
use anyhow::{anyhow, Result};
use fission_core::action::AppState;
use fission_core::event::{ImeEvent, InputEvent, KeyCode, KeyEvent, PointerButton, PointerEvent};
use fission_ir::{LayoutOp, NodeId, Op};
use fission_layout::{LayoutPoint, LayoutRect, LayoutSize};
use fission_render::DisplayOp;

#[derive(Debug, Clone)]
pub struct TextMatch {
    pub text: String,
    pub bounds: LayoutRect,
    pub node_id: Option<NodeId>,
}

#[derive(Debug, Clone)]
pub struct SemanticMatch {
    pub role: fission_ir::semantics::Role,
    pub label: Option<String>,
    pub bounds: LayoutRect,
    pub node_id: NodeId,
}

pub struct TestDriver<S: AppState> {
    pub harness: TestHarness<S>,
    auto_pump: bool,
}

impl<S: AppState> TestDriver<S> {
    pub fn new(harness: TestHarness<S>) -> Self {
        Self {
            harness,
            auto_pump: true,
        }
    }

    pub fn set_viewport(&mut self, width: f32, height: f32) {
        self.harness.env.viewport_size = LayoutSize::new(width, height);
    }

    pub fn pump(&mut self) -> Result<()> {
        self.harness.pump()
    }

    // --- Queries ---

    pub fn find_text(&self, needle: &str) -> Option<TextMatch> {
        self.find_all_text(needle).into_iter().next()
    }

    pub fn find_all_text(&self, needle: &str) -> Vec<TextMatch> {
        let dl = match self.harness.get_last_display_list() {
            Some(dl) => dl,
            None => return vec![],
        };
        let mut results = Vec::new();
        for op in &dl.ops {
            match op {
                DisplayOp::DrawText {
                    text,
                    bounds,
                    node_id,
                    ..
                } => {
                    if text.contains(needle) {
                        results.push(TextMatch {
                            text: text.clone(),
                            bounds: *bounds,
                            node_id: *node_id,
                        });
                    }
                }
                DisplayOp::DrawRichText {
                    runs,
                    bounds,
                    node_id,
                    ..
                } => {
                    let combined: String = runs.iter().map(|r| r.text.clone()).collect();
                    if combined.contains(needle) {
                        results.push(TextMatch {
                            text: combined,
                            bounds: *bounds,
                            node_id: *node_id,
                        });
                    }
                }
                _ => {}
            }
        }
        results
    }

    pub fn find_role(&self, role: fission_ir::semantics::Role) -> Vec<SemanticMatch> {
        let ir = match &self.harness.last_ir {
            Some(ir) => ir,
            None => return vec![],
        };
        let snapshot = match &self.harness.last_snapshot {
            Some(s) => s,
            None => return vec![],
        };
        let mut results = Vec::new();
        for (id, node) in &ir.nodes {
            if let Op::Semantics(sem) = &node.op {
                if sem.role == role {
                    let bounds = snapshot
                        .get_node_rect(*id)
                        .unwrap_or(LayoutRect::new(0.0, 0.0, 0.0, 0.0));
                    results.push(SemanticMatch {
                        role: sem.role,
                        label: sem.label.clone(),
                        bounds,
                        node_id: *id,
                    });
                }
            }
        }
        results
    }

    pub fn get_all_visible_text(&self) -> Vec<String> {
        let dl = match self.harness.get_last_display_list() {
            Some(dl) => dl,
            None => return vec![],
        };
        let mut texts = Vec::new();
        for op in &dl.ops {
            match op {
                DisplayOp::DrawText { text, .. } => texts.push(text.clone()),
                DisplayOp::DrawRichText { runs, .. } => {
                    texts.push(runs.iter().map(|r| r.text.clone()).collect());
                }
                _ => {}
            }
        }
        texts
    }

    // --- Interactions ---

    pub fn tap_point(&mut self, x: f32, y: f32) -> Result<()> {
        let point = LayoutPoint::new(x, y);
        self.harness
            .send_event(InputEvent::Pointer(PointerEvent::Down {
                point,
                button: PointerButton::Primary,
                modifiers: 0,
            }))?;
        self.harness
            .send_event(InputEvent::Pointer(PointerEvent::Up {
                point,
                button: PointerButton::Primary,
                modifiers: 0,
            }))?;
        if self.auto_pump {
            self.harness.pump()?;
        }
        Ok(())
    }

    pub fn tap_text(&mut self, needle: &str) -> Result<()> {
        let m = self
            .find_text(needle)
            .ok_or_else(|| anyhow!("text '{}' not found in display list", needle))?;
        let cx = m.bounds.x() + m.bounds.width() / 2.0;
        let cy = m.bounds.y() + m.bounds.height() / 2.0;
        self.tap_point(cx, cy)
    }

    pub fn scroll(&mut self, at: LayoutPoint, delta: LayoutPoint) -> Result<()> {
        self.harness
            .send_event(InputEvent::Pointer(PointerEvent::Scroll {
                point: at,
                delta,
                modifiers: 0,
            }))?;
        if self.auto_pump {
            self.harness.pump()?;
        }
        Ok(())
    }

    pub fn scroll_down(&mut self, at: LayoutPoint, pixels: f32) -> Result<()> {
        self.scroll(at, LayoutPoint::new(0.0, pixels))
    }

    pub fn scroll_to_text(&mut self, needle: &str) -> Result<()> {
        // Find the text
        let text_match = self
            .find_text(needle)
            .ok_or_else(|| anyhow!("text '{}' not found", needle))?;

        // Find the text's node_id, then walk up the IR to find enclosing Scroll
        let ir = self
            .harness
            .last_ir
            .as_ref()
            .ok_or_else(|| anyhow!("no IR"))?;
        let snapshot = self
            .harness
            .last_snapshot
            .as_ref()
            .ok_or_else(|| anyhow!("no snapshot"))?;

        let text_node_id = text_match
            .node_id
            .ok_or_else(|| anyhow!("text has no node_id"))?;

        // Walk up parents to find Scroll container
        let mut current = ir.nodes.get(&text_node_id).and_then(|n| n.parent);
        let mut scroll_id = None;
        while let Some(pid) = current {
            if let Some(pnode) = ir.nodes.get(&pid) {
                if matches!(pnode.op, Op::Layout(LayoutOp::Scroll { .. })) {
                    scroll_id = Some(pid);
                    break;
                }
                current = pnode.parent;
            } else {
                break;
            }
        }

        let scroll_id =
            scroll_id.ok_or_else(|| anyhow!("no enclosing Scroll found for '{}'", needle))?;
        let scroll_rect = snapshot
            .get_node_rect(scroll_id)
            .ok_or_else(|| anyhow!("scroll node has no rect"))?;

        // Calculate needed offset to bring text into view
        let text_y = text_match.bounds.y();
        let viewport_top = scroll_rect.y();
        let viewport_bottom = viewport_top + scroll_rect.height();

        if text_y >= viewport_top && text_y + text_match.bounds.height() <= viewport_bottom {
            return Ok(()); // Already visible
        }

        // Set scroll offset directly for reliability
        let needed_offset = (text_y - viewport_top - 20.0).max(0.0); // 20px margin from top
        self.harness
            .runtime
            .runtime_state
            .scroll
            .set_offset(scroll_id, needed_offset);
        self.harness.pump()?;
        Ok(())
    }

    pub fn type_text(&mut self, text: &str) -> Result<()> {
        for ch in text.chars() {
            if ch.is_ascii() {
                let key_code = if ch == ' ' {
                    KeyCode::Space
                } else if ch == '\n' {
                    KeyCode::Enter
                } else {
                    KeyCode::Char(ch)
                };
                self.harness
                    .send_event(InputEvent::Keyboard(KeyEvent::Down {
                        key_code: key_code.clone(),
                        modifiers: 0,
                    }))?;
            } else {
                // Non-ASCII: use IME commit
                self.harness.send_event(InputEvent::Ime(ImeEvent::Commit {
                    text: ch.to_string(),
                }))?;
            }
        }
        if self.auto_pump {
            self.harness.pump()?;
        }
        Ok(())
    }

    pub fn press_key(&mut self, key: KeyCode, modifiers: u8) -> Result<()> {
        self.harness
            .send_event(InputEvent::Keyboard(KeyEvent::Down {
                key_code: key,
                modifiers,
            }))?;
        if self.auto_pump {
            self.harness.pump()?;
        }
        Ok(())
    }

    pub fn tick(&mut self, dt_ms: u64) -> Result<()> {
        self.harness.tick(dt_ms)?;
        if self.auto_pump {
            self.harness.pump()?;
        }
        Ok(())
    }

    // --- Assertions ---

    pub fn assert_text_visible(&self, needle: &str) {
        assert!(
            self.find_text(needle).is_some(),
            "expected text '{}' to be visible, but it was not found in the display list",
            needle
        );
    }

    pub fn assert_text_not_visible(&self, needle: &str) {
        assert!(
            self.find_text(needle).is_none(),
            "expected text '{}' to NOT be visible, but it was found",
            needle
        );
    }
}
