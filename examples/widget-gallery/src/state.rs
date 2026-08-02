use fission::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GalleryState {
    pub slider_value: f32,
    pub colour_value: Color,
    pub colour_variant: usize,
    pub checked: bool,
    pub switch_on: bool,
    pub text_value: String,
    pub number_value: f32,
    pub active_tab: usize,
    pub accordion_open: usize,
    pub select_open: bool,
    pub select_value: Option<String>,
    pub menu_open: bool,
    pub modal_open: bool,
    pub drawer_open: bool,
    pub segmented_index: usize,
    pub current_page: usize,
    pub tree_expanded: HashSet<String>,
    pub tree_selected: Option<String>,
    pub show_toast: bool,
    pub drag_backlog: Vec<String>,
    pub drag_done: Vec<String>,
    pub drag_hover_zone: Option<String>,
    pub drag_log: Vec<String>,
    pub drag_snap_preview: bool,
    pub drag_external_files: Vec<String>,
}

impl Default for GalleryState {
    fn default() -> Self {
        let mut tree_expanded = HashSet::new();
        tree_expanded.insert("src".into());

        Self {
            slider_value: 50.0,
            colour_value: Color {
                r: 59,
                g: 130,
                b: 246,
                a: 255,
            },
            colour_variant: 0,
            checked: true,
            switch_on: true,
            text_value: String::new(),
            number_value: 5.0,
            active_tab: 0,
            accordion_open: 0,
            select_open: false,
            select_value: None,
            menu_open: false,
            modal_open: false,
            drawer_open: false,
            segmented_index: 0,
            current_page: 1,
            tree_expanded,
            tree_selected: None,
            show_toast: false,
            drag_backlog: vec![
                "Inbox triage".into(),
                "Invoice import".into(),
                "Follow-up".into(),
            ],
            drag_done: vec!["Release notes".into()],
            drag_hover_zone: None,
            drag_log: Vec::new(),
            drag_snap_preview: false,
            drag_external_files: Vec::new(),
        }
    }
}

impl GlobalState for GalleryState {}
