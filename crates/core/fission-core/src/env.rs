use crate::{action::AppState, registry::{AnimationPropertyId, EasingFunction}};
use fission_i18n::{I18nRegistry, Locale};
use fission_ir::{NodeId, WidgetNodeId};
use fission_layout::{LayoutPoint, LayoutSize};
use fission_text_engine::{EditTransaction, TextBuffer, TextEdit};
use fission_theme::Theme;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct WindowInsets {
    pub top: f32,
    pub bottom: f32,
    pub left: f32,
    pub right: f32,
}

// Static environment data (Theme, I18n)
#[derive(Clone)]
pub struct Env {
    pub theme: Theme,
    pub i18n: I18nRegistry,
    pub locale: Locale,
    pub window_insets: WindowInsets,
    pub viewport_size: LayoutSize,
    pub measurer: Option<Arc<dyn fission_layout::TextMeasurer>>,
}

impl Default for Env {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            i18n: I18nRegistry::new(),
            locale: Locale::default(),
            window_insets: WindowInsets::default(),
            viewport_size: LayoutSize::default(),
            measurer: None,
        }
    }
}

impl std::fmt::Debug for Env {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Env")
            .field("theme", &self.theme)
            .field("locale", &self.locale)
            .field("window_insets", &self.window_insets)
            .field("viewport_size", &self.viewport_size)
            .finish()
    }
}

impl Env {
    pub fn new(measurer: Arc<dyn fission_layout::TextMeasurer>) -> Self {
        Self {
            theme: Theme::default(),
            i18n: I18nRegistry::new(),
            locale: Locale::default(),
            window_insets: WindowInsets::default(),
            viewport_size: LayoutSize::default(),
            measurer: Some(measurer),
        }
    }
}

pub trait Clipboard: Send + Sync {
    fn get_text(&self) -> Option<String>;
    fn set_text(&self, text: &str);
}

pub trait ImeHandler: Send + Sync {
    fn set_ime_allowed(&self, allowed: bool);
    fn set_ime_cursor_area(&self, rect: fission_layout::LayoutRect);
}

// Runtime state managed by framework (Interaction)
#[derive(Clone, Debug, Default)]
pub struct RuntimeState {
    pub scroll: ScrollStateMap,
    pub video: VideoStateMap,
    pub web: WebStateMap,
    pub animation: AnimationStateMap,
    pub interaction: InteractionStateMap,
    pub text_edit: TextEditStateMap,
    pub clipboard: String,
    pub caret_visible: HashMap<NodeId, bool>,
    pub gesture: GestureState,
    pub hero: HeroState,
}

#[derive(Clone, Debug, Default)]
pub struct HeroState {
    // tag -> (Last Known NodeId, Last Known Rect)
    pub positions: HashMap<String, (NodeId, fission_layout::LayoutRect)>,
}

#[derive(Clone, Debug, Default)]
pub struct GestureState {
    pub start_point: Option<LayoutPoint>,
    pub last_point: Option<LayoutPoint>,
    pub is_panning: bool,
    pub target_node: Option<NodeId>,
    pub dragging_payload: Option<Vec<u8>>,
    pub pressed_button: Option<crate::event::PointerButton>,
}

#[derive(Clone, Debug, Default)]
pub struct AnimationStateMap {
    pub values: HashMap<(WidgetNodeId, AnimationPropertyId), f32>,
    pub active: HashMap<(WidgetNodeId, AnimationPropertyId), ActiveAnimation>,
}

#[derive(Clone, Debug)]
pub struct ActiveAnimation {
    pub target: WidgetNodeId,
    pub property: AnimationPropertyId,
    pub start_value: f32,
    pub end_value: f32,
    pub start_time: u64,
    pub duration: u64,
    pub repeat: bool,
    pub frame_interval_ms: Option<u64>,
    pub easing: EasingFunction,
}

#[derive(Clone, Debug, Default)]
pub struct ScrollStateMap {
    pub offsets: HashMap<NodeId, f32>,
}

impl ScrollStateMap {
    pub fn get_offset(&self, id: NodeId) -> f32 {
        *self.offsets.get(&id).unwrap_or(&0.0)
    }

    pub fn set_offset(&mut self, id: NodeId, offset: f32) {
        self.offsets.insert(id, offset);
    }
}

#[derive(Clone, Debug, Default)]
pub struct TextEditStateMap {
    pub states: HashMap<NodeId, TextEditState>,
}

#[derive(Clone, Debug)]
pub struct TextEditState {
    pub buffer: TextBuffer,
    pub caret: usize,  // byte index into value
    pub anchor: usize, // selection anchor; if equal to caret then no selection
    pub history: TextEditHistory,
    pub preedit: Option<TextPreeditState>,
    pub pending_model_sync: bool, // True when edits are newer than the currently lowered semantics value
    /// Last cursor position that was dispatched as a CursorChanged action.
    /// Used to deduplicate dispatches and prevent unnecessary model updates
    /// that could cause extra rebuild cycles.
    pub last_dispatched_cursor: Option<(usize, usize)>,
}

impl Default for TextEditState {
    fn default() -> Self {
        Self {
            buffer: TextBuffer::new(),
            caret: 0,
            anchor: 0,
            history: TextEditHistory::default(),
            preedit: None,
            pending_model_sync: false,
            last_dispatched_cursor: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct TextPreeditState {
    pub text: String,
    pub range: (usize, usize),
}

#[derive(Clone, Debug)]
pub struct TextHistoryEntry {
    pub transaction: EditTransaction,
    pub before_caret: usize,
    pub before_anchor: usize,
    pub after_caret: usize,
    pub after_anchor: usize,
}

#[derive(Clone, Debug)]
pub struct TextEditHistory {
    pub undo_stack: Vec<TextHistoryEntry>,
    pub redo_stack: Vec<TextHistoryEntry>,
    pub capacity: usize, // Max undo steps
}

impl Default for TextEditHistory {
    fn default() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            capacity: 100,
        }
    }
}

impl TextEditHistory {
    pub fn record(&mut self, entry: TextHistoryEntry) {
        self.undo_stack.push(entry);
        if self.undo_stack.len() > self.capacity {
            let overflow = self.undo_stack.len() - self.capacity;
            self.undo_stack.drain(0..overflow);
        }
        self.redo_stack.clear();
    }

    pub fn undo(&mut self, buffer: &mut TextBuffer) -> Option<(usize, usize)> {
        let entry = self.undo_stack.pop()?;
        apply_transaction(buffer, &entry.transaction.inverse());
        let caret = entry.before_caret;
        let anchor = entry.before_anchor;
        self.redo_stack.push(entry);
        Some((caret, anchor))
    }

    pub fn redo(&mut self, buffer: &mut TextBuffer) -> Option<(usize, usize)> {
        let entry = self.redo_stack.pop()?;
        apply_transaction(buffer, &entry.transaction);
        let caret = entry.after_caret;
        let anchor = entry.after_anchor;
        self.undo_stack.push(entry);
        Some((caret, anchor))
    }
}

fn apply_transaction(buffer: &mut TextBuffer, transaction: &EditTransaction) {
    for edit in &transaction.edits {
        buffer.replace(edit.range.clone(), &edit.new_text);
    }
}

impl TextEditStateMap {
    pub fn get_mut_or_default(&mut self, id: NodeId) -> &mut TextEditState {
        self.states.entry(id).or_default()
    }
    pub fn get(&self, id: NodeId) -> Option<&TextEditState> {
        self.states.get(&id)
    }
    pub fn set_caret(&mut self, id: NodeId, caret: usize, anchor: Option<usize>) {
        let st = self.states.entry(id).or_default();
        st.caret = caret;
        st.anchor = anchor.unwrap_or(caret);
        st.pending_model_sync = false;
    }
}

impl TextEditState {
    pub fn committed_text(&self) -> String {
        self.buffer.to_string()
    }

    pub fn sync_from_model(&mut self, semantic_value: &str) {
        if self.pending_model_sync && self.buffer.to_string() == semantic_value {
            self.pending_model_sync = false;
        }

        if !self.pending_model_sync && self.buffer.to_string() != semantic_value {
            self.buffer = TextBuffer::from_str(semantic_value);
            self.caret = self.caret.min(semantic_value.len());
            self.anchor = self.anchor.min(semantic_value.len());
            self.preedit = None;
            self.history = TextEditHistory::default();
        }
    }

    pub fn selection_range(&self) -> (usize, usize) {
        if self.caret <= self.anchor {
            (self.caret, self.anchor)
        } else {
            (self.anchor, self.caret)
        }
    }

    pub fn clear_preedit(&mut self) {
        self.preedit = None;
    }

    pub fn set_preedit(&mut self, text: String) {
        if text.is_empty() {
            self.preedit = None;
            return;
        }

        if let Some(preedit) = &mut self.preedit {
            preedit.text = text;
            return;
        }

        self.preedit = Some(TextPreeditState {
            text,
            range: self.selection_range(),
        });
    }

    pub fn display_text(&self) -> (String, Option<(usize, usize)>) {
        let committed = self.buffer.to_string();
        let Some(preedit) = &self.preedit else {
            return (committed, None);
        };

        let start = preedit.range.0.min(committed.len());
        let end = preedit.range.1.min(committed.len());

        let mut display = String::with_capacity(
            committed.len() - (end.saturating_sub(start)) + preedit.text.len(),
        );
        display.push_str(&committed[..start]);
        display.push_str(&preedit.text);
        display.push_str(&committed[end..]);
        (display, Some((start, start + preedit.text.len())))
    }

    pub fn apply_edit(
        &mut self,
        range: std::ops::Range<usize>,
        new_text: &str,
        next_caret: usize,
        next_anchor: usize,
    ) -> String {
        let old_text = self.buffer.slice(range.clone()).to_string();
        let mut txn = EditTransaction::new();
        txn.push(TextEdit::new(range, new_text, old_text));
        apply_transaction(&mut self.buffer, &txn);
        self.history.record(TextHistoryEntry {
            transaction: txn,
            before_caret: self.caret,
            before_anchor: self.anchor,
            after_caret: next_caret,
            after_anchor: next_anchor,
        });
        self.caret = next_caret;
        self.anchor = next_anchor;
        self.preedit = None;
        self.pending_model_sync = true;
        self.buffer.to_string()
    }

    pub fn undo(&mut self) -> Option<(String, usize, usize)> {
        let (caret, anchor) = self.history.undo(&mut self.buffer)?;
        self.caret = caret;
        self.anchor = anchor;
        self.preedit = None;
        self.pending_model_sync = true;
        Some((self.buffer.to_string(), caret, anchor))
    }

    pub fn redo(&mut self) -> Option<(String, usize, usize)> {
        let (caret, anchor) = self.history.redo(&mut self.buffer)?;
        self.caret = caret;
        self.anchor = anchor;
        self.preedit = None;
        self.pending_model_sync = true;
        Some((self.buffer.to_string(), caret, anchor))
    }
}

#[derive(Clone, Debug, Default)]
pub struct InteractionStateMap {
    pub hovered: HashMap<NodeId, bool>,
    pub pressed: HashMap<NodeId, bool>,
    pub focused: Option<NodeId>,
    pub last_down_point: Option<LayoutPoint>,
}

impl InteractionStateMap {
    pub fn is_hovered(&self, id: NodeId) -> bool {
        self.hovered.get(&id).copied().unwrap_or(false)
    }
    pub fn is_pressed(&self, id: NodeId) -> bool {
        self.pressed.get(&id).copied().unwrap_or(false)
    }
    pub fn is_focused(&self, id: NodeId) -> bool {
        self.focused == Some(id)
    }

    pub fn set_hovered(&mut self, id: NodeId, value: bool) {
        if value {
            self.hovered.insert(id, true);
        } else {
            self.hovered.remove(&id);
        }
    }

    pub fn set_pressed(&mut self, id: NodeId, value: bool) {
        if value {
            self.pressed.insert(id, true);
        } else {
            self.pressed.remove(&id);
        }
    }

    pub fn set_focused(&mut self, id: Option<NodeId>) {
        self.focused = id;
    }
}

#[derive(Clone, Debug, Default)]
pub struct VideoStateMap {
    pub states: HashMap<WidgetNodeId, VideoState>,
}

#[derive(Clone, Debug, Default)]
pub struct WebState {
    pub url: String,
    pub user_agent: Option<String>,
    pub loading: bool,
    pub can_go_back: bool,
    pub can_go_forward: bool,
    pub title: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct WebStateMap {
    pub states: HashMap<WidgetNodeId, WebState>,
}

// Static environment data (Theme, I18n)

impl AppState for VideoStateMap {}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VideoState {
    pub status: VideoStatus,
    pub position_ms: u64,
    pub duration_ms: Option<u64>,
    pub rate: f32,
    pub volume: f32,
    pub muted: bool,
    pub looped: bool,
    pub asset_source: String,
    pub surface_id: Option<u64>,
    pub pending_seek: Option<u64>,
}

impl Default for VideoState {
    fn default() -> Self {
        Self {
            status: VideoStatus::Stopped,
            position_ms: 0,
            duration_ms: None,
            rate: 1.0,
            volume: 1.0,
            muted: false,
            looped: false,
            asset_source: String::new(),
            surface_id: None,
            pending_seek: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum VideoStatus {
    Stopped,
    Playing,
    Paused,
    Buffering,
    Ended,
    Error,
}
