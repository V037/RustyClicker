use super::{
    BUTTON_STYLE,
    controls::create_control,
    layout::{move_control, scale},
    state::{AppState, set_text},
};
use serde::{Deserialize, Serialize};
use std::{ffi::OsString, fs, os::windows::ffi::OsStringExt, path::PathBuf};
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, WPARAM},
        UI::{
            Controls::{
                Dialogs::{
                    GetOpenFileNameW, GetSaveFileNameW, OFN_FILEMUSTEXIST, OFN_OVERWRITEPROMPT,
                    OFN_PATHMUSTEXIST, OPENFILENAMEW,
                },
                HTREEITEM, TVI_LAST, TVI_ROOT, TVIF_PARAM, TVIF_TEXT, TVINSERTSTRUCTW,
                TVINSERTSTRUCTW_0, TVITEMW, TVM_DELETEITEM, TVM_INSERTITEMW, TVS_HASBUTTONS,
                TVS_HASLINES, TVS_LINESATROOT, TVS_SHOWSELALWAYS,
            },
            WindowsAndMessaging::{
                ES_AUTOHSCROLL, GetWindowTextW, MB_ICONERROR, MB_ICONWARNING, SW_HIDE, SW_SHOW,
                SendMessageW, ShowWindow, WINDOW_STYLE, WS_BORDER, WS_CHILD, WS_VISIBLE,
            },
        },
    },
    core::{PCWSTR, PWSTR, w},
};

pub(crate) const ID_LOGIC_BUTTON: usize = 15;
pub(crate) const ID_LOGIC_TREE_VIEW: usize = 400;
pub(crate) const ID_LOGIC_ADD_CLICK_BTN: usize = 401;
pub(crate) const ID_LOGIC_ADD_REPEAT_BTN: usize = 402;
pub(crate) const ID_LOGIC_ADD_IFELSE_BTN: usize = 403;
pub(crate) const ID_LOGIC_DELETE_BTN: usize = 404;
pub(crate) const ID_LOGIC_PROP1_EDIT: usize = 405;
pub(crate) const ID_LOGIC_PROP2_EDIT: usize = 406;
pub(crate) const ID_LOGIC_PROP3_EDIT: usize = 407;
pub(crate) const ID_LOGIC_APPLY_BTN: usize = 408;
pub(crate) const ID_LOGIC_SAVE_BTN: usize = 409;
pub(crate) const ID_LOGIC_LOAD_BTN: usize = 410;
pub(crate) const ID_LOGIC_TREE_TITLE: usize = 412;
pub(crate) const ID_LOGIC_PROPS_TITLE: usize = 413;
pub(crate) const ID_LOGIC_STATUS: usize = 414;
pub(crate) const ID_LOGIC_NODE_TYPE_LBL: usize = 415;
pub(crate) const ID_LOGIC_PROP1_LBL: usize = 416;
pub(crate) const ID_LOGIC_PROP2_LBL: usize = 417;
pub(crate) const ID_LOGIC_PROP3_LBL: usize = 418;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum BehaviorNode {
    ClickImage {
        slot: usize,
        confidence: f32,
        delay_ms: u64,
    },
    Repeat {
        times: u32,
        children: Vec<BehaviorNode>,
    },
    IfElse {
        condition: String,
        then_branch: Vec<BehaviorNode>,
        else_branch: Vec<BehaviorNode>,
    },
    Sequence {
        name: String,
        children: Vec<BehaviorNode>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub(crate) struct BehaviorTreeDocument {
    pub(crate) name: String,
    pub(crate) root: BehaviorNode,
}

impl Default for BehaviorTreeDocument {
    fn default() -> Self {
        Self {
            name: "Default Behavior Tree".to_owned(),
            root: BehaviorNode::Sequence {
                name: "Main Sequence".to_owned(),
                children: vec![
                    BehaviorNode::ClickImage {
                        slot: 1,
                        confidence: 0.90,
                        delay_ms: 100,
                    },
                    BehaviorNode::Repeat {
                        times: 3,
                        children: vec![BehaviorNode::ClickImage {
                            slot: 2,
                            confidence: 0.90,
                            delay_ms: 150,
                        }],
                    },
                    BehaviorNode::IfElse {
                        condition: "image_found".to_owned(),
                        then_branch: vec![BehaviorNode::ClickImage {
                            slot: 3,
                            confidence: 0.95,
                            delay_ms: 200,
                        }],
                        else_branch: Vec::new(),
                    },
                ],
            },
        }
    }
}

/// Represents an item in the tree model with parent tracking for TreeView navigation.
#[derive(Clone, Debug)]
pub(crate) struct NodeItem {
    pub(crate) id: usize,
    pub(crate) parent_id: Option<usize>,
    pub(crate) node: BehaviorNode,
}

#[derive(Clone, Debug)]
pub(crate) struct LogicState {
    pub(crate) next_id: usize,
    pub(crate) items: Vec<NodeItem>,
}

impl Default for LogicState {
    fn default() -> Self {
        let mut state = Self {
            next_id: 1,
            items: Vec::new(),
        };
        let doc = BehaviorTreeDocument::default();
        state.load_from_document(&doc);
        state
    }
}

impl LogicState {
    pub(crate) fn load_from_document(&mut self, doc: &BehaviorTreeDocument) {
        self.items.clear();
        self.next_id = 1;
        self.import_node_recursive(&doc.root, None);
    }

    fn import_node_recursive(&mut self, node: &BehaviorNode, parent_id: Option<usize>) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        self.items.push(NodeItem {
            id,
            parent_id,
            node: node.clone(),
        });

        match node {
            BehaviorNode::ClickImage { .. } => {}
            BehaviorNode::Repeat { children, .. } => {
                for child in children {
                    self.import_node_recursive(child, Some(id));
                }
            }
            BehaviorNode::Sequence { children, .. } => {
                for child in children {
                    self.import_node_recursive(child, Some(id));
                }
            }
            BehaviorNode::IfElse {
                then_branch,
                else_branch,
                ..
            } => {
                for child in then_branch {
                    self.import_node_recursive(child, Some(id));
                }
                for child in else_branch {
                    self.import_node_recursive(child, Some(id));
                }
            }
        }
        id
    }

    pub(crate) fn add_node(&mut self, parent_id: Option<usize>, node: BehaviorNode) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        self.items.push(NodeItem {
            id,
            parent_id,
            node,
        });
        id
    }

    pub(crate) fn delete_node(&mut self, id: usize) {
        let mut to_remove = Vec::new();
        self.collect_descendant_ids(id, &mut to_remove);
        to_remove.push(id);
        self.items.retain(|item| !to_remove.contains(&item.id));
    }

    fn collect_descendant_ids(&self, parent_id: usize, list: &mut Vec<usize>) {
        for item in &self.items {
            if item.parent_id == Some(parent_id) {
                list.push(item.id);
                self.collect_descendant_ids(item.id, list);
            }
        }
    }

    pub(crate) fn get_item(&self, id: usize) -> Option<&NodeItem> {
        self.items.iter().find(|item| item.id == id)
    }

    pub(crate) fn get_item_mut(&mut self, id: usize) -> Option<&mut NodeItem> {
        self.items.iter_mut().find(|item| item.id == id)
    }

    pub(crate) fn to_document(&self) -> BehaviorTreeDocument {
        let root_item = self.items.iter().find(|item| item.parent_id.is_none());
        let root_node = match root_item {
            Some(item) => self.export_node_recursive(item.id),
            None => BehaviorNode::Sequence {
                name: "Empty Root".to_owned(),
                children: Vec::new(),
            },
        };
        BehaviorTreeDocument {
            name: "Exported Behavior Tree".to_owned(),
            root: root_node,
        }
    }

    fn export_node_recursive(&self, id: usize) -> BehaviorNode {
        let item = self.items.iter().find(|i| i.id == id).cloned();
        let Some(item) = item else {
            return BehaviorNode::Sequence {
                name: "Unknown".to_owned(),
                children: Vec::new(),
            };
        };

        let direct_children: Vec<usize> = self
            .items
            .iter()
            .filter(|i| i.parent_id == Some(id))
            .map(|i| i.id)
            .collect();

        match item.node {
            BehaviorNode::ClickImage {
                slot,
                confidence,
                delay_ms,
            } => BehaviorNode::ClickImage {
                slot,
                confidence,
                delay_ms,
            },
            BehaviorNode::Repeat { times, .. } => {
                let children = direct_children
                    .into_iter()
                    .map(|cid| self.export_node_recursive(cid))
                    .collect();
                BehaviorNode::Repeat { times, children }
            }
            BehaviorNode::Sequence { name, .. } => {
                let children = direct_children
                    .into_iter()
                    .map(|cid| self.export_node_recursive(cid))
                    .collect();
                BehaviorNode::Sequence { name, children }
            }
            BehaviorNode::IfElse { condition, .. } => {
                let mut then_branch = Vec::new();
                for cid in direct_children {
                    then_branch.push(self.export_node_recursive(cid));
                }
                BehaviorNode::IfElse {
                    condition,
                    then_branch,
                    else_branch: Vec::new(),
                }
            }
        }
    }
}

fn node_display_label(node: &BehaviorNode) -> String {
    match node {
        BehaviorNode::ClickImage { slot, delay_ms, .. } => {
            format!("Click Image [Slot {slot}, {delay_ms}ms]")
        }
        BehaviorNode::Repeat { times, .. } => {
            if *times == 0 {
                "Repeat [Infinite]".to_owned()
            } else {
                format!("Repeat [{times} times]")
            }
        }
        BehaviorNode::IfElse { condition, .. } => {
            format!("If / Else [{condition}]")
        }
        BehaviorNode::Sequence { name, .. } => {
            format!("Sequence: {name}")
        }
    }
}

pub(crate) unsafe fn create_logic_controls(window: HWND, state: &mut AppState) {
    unsafe {
        state.logic_tree_title = create_control(
            window,
            w!("STATIC"),
            "BEHAVIOR TREE",
            WINDOW_STYLE(0),
            18,
            55,
            200,
            20,
            ID_LOGIC_TREE_TITLE,
        );

        state.logic_tree_view = create_control(
            window,
            w!("SysTreeView32"),
            "",
            WINDOW_STYLE(
                WS_CHILD.0
                    | WS_VISIBLE.0
                    | WS_BORDER.0
                    | TVS_HASBUTTONS
                    | TVS_HASLINES
                    | TVS_LINESATROOT
                    | TVS_SHOWSELALWAYS,
            ),
            18,
            80,
            220,
            230,
            ID_LOGIC_TREE_VIEW,
        );

        state.logic_add_click_btn = create_control(
            window,
            w!("BUTTON"),
            "+ Click",
            WINDOW_STYLE(BUTTON_STYLE),
            18,
            318,
            66,
            28,
            ID_LOGIC_ADD_CLICK_BTN,
        );
        state.logic_add_repeat_btn = create_control(
            window,
            w!("BUTTON"),
            "+ Repeat",
            WINDOW_STYLE(BUTTON_STYLE),
            88,
            318,
            68,
            28,
            ID_LOGIC_ADD_REPEAT_BTN,
        );
        state.logic_add_ifelse_btn = create_control(
            window,
            w!("BUTTON"),
            "+ If/Else",
            WINDOW_STYLE(BUTTON_STYLE),
            160,
            318,
            78,
            28,
            ID_LOGIC_ADD_IFELSE_BTN,
        );
        state.logic_delete_btn = create_control(
            window,
            w!("BUTTON"),
            "Delete",
            WINDOW_STYLE(BUTTON_STYLE),
            18,
            352,
            70,
            28,
            ID_LOGIC_DELETE_BTN,
        );

        state.logic_props_title = create_control(
            window,
            w!("STATIC"),
            "NODE PROPERTIES",
            WINDOW_STYLE(0),
            250,
            55,
            200,
            20,
            ID_LOGIC_PROPS_TITLE,
        );

        state.logic_node_type_label = create_control(
            window,
            w!("STATIC"),
            "Select a node to inspect properties",
            WINDOW_STYLE(0),
            250,
            80,
            200,
            22,
            ID_LOGIC_NODE_TYPE_LBL,
        );

        state.logic_prop1_label = create_control(
            window,
            w!("STATIC"),
            "Parameter 1:",
            WINDOW_STYLE(0),
            250,
            110,
            180,
            18,
            ID_LOGIC_PROP1_LBL,
        );
        state.logic_prop1_edit = create_control(
            window,
            w!("EDIT"),
            "",
            WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | WS_BORDER.0 | ES_AUTOHSCROLL as u32),
            250,
            130,
            180,
            24,
            ID_LOGIC_PROP1_EDIT,
        );

        state.logic_prop2_label = create_control(
            window,
            w!("STATIC"),
            "Parameter 2:",
            WINDOW_STYLE(0),
            250,
            160,
            180,
            18,
            ID_LOGIC_PROP2_LBL,
        );
        state.logic_prop2_edit = create_control(
            window,
            w!("EDIT"),
            "",
            WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | WS_BORDER.0 | ES_AUTOHSCROLL as u32),
            250,
            180,
            180,
            24,
            ID_LOGIC_PROP2_EDIT,
        );

        state.logic_prop3_label = create_control(
            window,
            w!("STATIC"),
            "Parameter 3:",
            WINDOW_STYLE(0),
            250,
            210,
            180,
            18,
            ID_LOGIC_PROP3_LBL,
        );
        state.logic_prop3_edit = create_control(
            window,
            w!("EDIT"),
            "",
            WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | WS_BORDER.0 | ES_AUTOHSCROLL as u32),
            250,
            230,
            180,
            24,
            ID_LOGIC_PROP3_EDIT,
        );

        state.logic_apply_btn = create_control(
            window,
            w!("BUTTON"),
            "Apply Changes",
            WINDOW_STYLE(BUTTON_STYLE),
            250,
            264,
            120,
            30,
            ID_LOGIC_APPLY_BTN,
        );

        state.logic_save_btn = create_control(
            window,
            w!("BUTTON"),
            "Export JSON",
            WINDOW_STYLE(BUTTON_STYLE),
            250,
            318,
            96,
            28,
            ID_LOGIC_SAVE_BTN,
        );
        state.logic_load_btn = create_control(
            window,
            w!("BUTTON"),
            "Import JSON",
            WINDOW_STYLE(BUTTON_STYLE),
            352,
            318,
            96,
            28,
            ID_LOGIC_LOAD_BTN,
        );

        state.logic_status_label = create_control(
            window,
            w!("STATIC"),
            "Ready. Add nodes or export tree as JSON.",
            WINDOW_STYLE(0),
            18,
            390,
            420,
            22,
            ID_LOGIC_STATUS,
        );

        state.logic_controls = vec![
            state.logic_tree_title,
            state.logic_tree_view,
            state.logic_add_click_btn,
            state.logic_add_repeat_btn,
            state.logic_add_ifelse_btn,
            state.logic_delete_btn,
            state.logic_props_title,
            state.logic_node_type_label,
            state.logic_prop1_label,
            state.logic_prop1_edit,
            state.logic_prop2_label,
            state.logic_prop2_edit,
            state.logic_prop3_label,
            state.logic_prop3_edit,
            state.logic_apply_btn,
            state.logic_save_btn,
            state.logic_load_btn,
            state.logic_status_label,
        ];

        for control in &state.logic_controls {
            let _ = ShowWindow(*control, SW_HIDE);
        }

        populate_treeview(state.logic_tree_view, &state.logic_state);
    }
}

pub(crate) fn populate_treeview(tree_hwnd: HWND, state: &LogicState) {
    unsafe {
        let _ = SendMessageW(tree_hwnd, TVM_DELETEITEM, Some(WPARAM(0)), Some(LPARAM(0)));
        let mut handle_map: Vec<(usize, HTREEITEM)> = Vec::new();

        for item in &state.items {
            let parent_handle = match item.parent_id {
                Some(pid) => handle_map
                    .iter()
                    .find(|(id, _)| *id == pid)
                    .map(|(_, h)| *h)
                    .unwrap_or(TVI_ROOT),
                None => TVI_ROOT,
            };

            let label = node_display_label(&item.node);
            let mut wide_label: Vec<u16> = label.encode_utf16().chain(Some(0)).collect();

            let tv_item = TVITEMW {
                mask: TVIF_TEXT | TVIF_PARAM,
                pszText: PWSTR(wide_label.as_mut_ptr()),
                lParam: LPARAM(item.id as isize),
                ..Default::default()
            };

            let insert = TVINSERTSTRUCTW {
                hParent: parent_handle,
                hInsertAfter: TVI_LAST,
                Anonymous: TVINSERTSTRUCTW_0 { item: tv_item },
            };

            let h_item = SendMessageW(
                tree_hwnd,
                TVM_INSERTITEMW,
                Some(WPARAM(0)),
                Some(LPARAM(&insert as *const _ as isize)),
            )
            .0;

            if h_item != 0 {
                handle_map.push((item.id, HTREEITEM(h_item)));
            }
        }
    }
}

pub(crate) fn layout_logic_controls(
    width: i32,
    height: i32,
    margin: i32,
    s: f32,
    state: &AppState,
) {
    let top_y = scale(55, s);
    let gap = scale(10, s);
    let content_width = (width - margin * 2).max(1);
    let left_width = (content_width - gap) / 2;
    let right_x = margin + left_width + gap;
    let right_width = content_width - left_width - gap;

    let tree_height = (height - top_y - scale(120, s)).max(scale(140, s));
    let buttons_y = top_y + tree_height + scale(8, s);
    let buttons2_y = buttons_y + scale(32, s);

    move_control(
        state.logic_tree_title,
        margin,
        top_y,
        left_width,
        scale(20, s),
    );
    move_control(
        state.logic_tree_view,
        margin,
        top_y + scale(24, s),
        left_width,
        tree_height,
    );

    let btn_w = (left_width - scale(8, s) * 2) / 3;
    move_control(
        state.logic_add_click_btn,
        margin,
        buttons_y,
        btn_w,
        scale(28, s),
    );
    move_control(
        state.logic_add_repeat_btn,
        margin + btn_w + scale(8, s),
        buttons_y,
        btn_w,
        scale(28, s),
    );
    move_control(
        state.logic_add_ifelse_btn,
        margin + (btn_w + scale(8, s)) * 2,
        buttons_y,
        left_width - (btn_w + scale(8, s)) * 2,
        scale(28, s),
    );
    move_control(
        state.logic_delete_btn,
        margin,
        buttons2_y,
        btn_w,
        scale(28, s),
    );

    move_control(
        state.logic_props_title,
        right_x,
        top_y,
        right_width,
        scale(20, s),
    );
    move_control(
        state.logic_node_type_label,
        right_x,
        top_y + scale(24, s),
        right_width,
        scale(22, s),
    );

    let mut py = top_y + scale(50, s);
    move_control(
        state.logic_prop1_label,
        right_x,
        py,
        right_width,
        scale(18, s),
    );
    py += scale(20, s);
    move_control(
        state.logic_prop1_edit,
        right_x,
        py,
        right_width,
        scale(24, s),
    );
    py += scale(30, s);

    move_control(
        state.logic_prop2_label,
        right_x,
        py,
        right_width,
        scale(18, s),
    );
    py += scale(20, s);
    move_control(
        state.logic_prop2_edit,
        right_x,
        py,
        right_width,
        scale(24, s),
    );
    py += scale(30, s);

    move_control(
        state.logic_prop3_label,
        right_x,
        py,
        right_width,
        scale(18, s),
    );
    py += scale(20, s);
    move_control(
        state.logic_prop3_edit,
        right_x,
        py,
        right_width,
        scale(24, s),
    );
    py += scale(32, s);

    move_control(
        state.logic_apply_btn,
        right_x,
        py,
        scale(130, s),
        scale(28, s),
    );

    let export_y = buttons_y;
    let export_w = (right_width - scale(8, s)) / 2;
    move_control(
        state.logic_save_btn,
        right_x,
        export_y,
        export_w,
        scale(28, s),
    );
    move_control(
        state.logic_load_btn,
        right_x + export_w + scale(8, s),
        export_y,
        right_width - export_w - scale(8, s),
        scale(28, s),
    );

    let status_y = height - scale(30, s);
    move_control(
        state.logic_status_label,
        margin,
        status_y,
        content_width,
        scale(20, s),
    );
}

impl AppState {
    #[allow(dead_code)]
    pub(crate) fn set_logic_open(&mut self, window: HWND, open: bool) {
        self.set_panel(
            window,
            if open {
                super::state::ActivePanel::Logic
            } else {
                super::state::ActivePanel::MainMenu
            },
        );
    }

    pub(crate) fn handle_logic_tree_selchanged(&mut self, node_id: usize) {
        self.logic_selected_node_id = Some(node_id);
        let Some(item) = self.logic_state.get_item(node_id) else {
            return;
        };

        match &item.node {
            BehaviorNode::ClickImage {
                slot,
                confidence,
                delay_ms,
            } => {
                set_text(self.logic_node_type_label, "Node: Click Image");
                set_text(self.logic_prop1_label, "Target Slot (1-99):");
                set_text(self.logic_prop1_edit, &slot.to_string());
                set_text(self.logic_prop2_label, "Confidence (0.1 - 1.0):");
                set_text(self.logic_prop2_edit, &format!("{confidence:.2}"));
                set_text(self.logic_prop3_label, "Post Delay (ms):");
                set_text(self.logic_prop3_edit, &delay_ms.to_string());
                unsafe {
                    let _ = ShowWindow(self.logic_prop2_label, SW_SHOW);
                    let _ = ShowWindow(self.logic_prop2_edit, SW_SHOW);
                    let _ = ShowWindow(self.logic_prop3_label, SW_SHOW);
                    let _ = ShowWindow(self.logic_prop3_edit, SW_SHOW);
                }
            }
            BehaviorNode::Repeat { times, .. } => {
                set_text(self.logic_node_type_label, "Node: Repeat Loop");
                set_text(self.logic_prop1_label, "Repeat Times (0 = infinite):");
                set_text(self.logic_prop1_edit, &times.to_string());
                unsafe {
                    let _ = ShowWindow(self.logic_prop2_label, SW_HIDE);
                    let _ = ShowWindow(self.logic_prop2_edit, SW_HIDE);
                    let _ = ShowWindow(self.logic_prop3_label, SW_HIDE);
                    let _ = ShowWindow(self.logic_prop3_edit, SW_HIDE);
                }
            }
            BehaviorNode::IfElse { condition, .. } => {
                set_text(self.logic_node_type_label, "Node: If / Else Branch");
                set_text(self.logic_prop1_label, "Condition String:");
                set_text(self.logic_prop1_edit, condition);
                unsafe {
                    let _ = ShowWindow(self.logic_prop2_label, SW_HIDE);
                    let _ = ShowWindow(self.logic_prop2_edit, SW_HIDE);
                    let _ = ShowWindow(self.logic_prop3_label, SW_HIDE);
                    let _ = ShowWindow(self.logic_prop3_edit, SW_HIDE);
                }
            }
            BehaviorNode::Sequence { name, .. } => {
                set_text(self.logic_node_type_label, "Node: Sequence");
                set_text(self.logic_prop1_label, "Sequence Name:");
                set_text(self.logic_prop1_edit, name);
                unsafe {
                    let _ = ShowWindow(self.logic_prop2_label, SW_HIDE);
                    let _ = ShowWindow(self.logic_prop2_edit, SW_HIDE);
                    let _ = ShowWindow(self.logic_prop3_label, SW_HIDE);
                    let _ = ShowWindow(self.logic_prop3_edit, SW_HIDE);
                }
            }
        }
    }

    pub(crate) fn handle_logic_command(&mut self, window: HWND, id: usize) {
        match id {
            ID_LOGIC_ADD_CLICK_BTN => {
                let parent = self.logic_selected_node_id;
                let new_node = BehaviorNode::ClickImage {
                    slot: 1,
                    confidence: 0.90,
                    delay_ms: 100,
                };
                let new_id = self.logic_state.add_node(parent, new_node);
                populate_treeview(self.logic_tree_view, &self.logic_state);
                self.handle_logic_tree_selchanged(new_id);
                set_text(self.logic_status_label, "Added 'Click Image' node.");
            }
            ID_LOGIC_ADD_REPEAT_BTN => {
                let parent = self.logic_selected_node_id;
                let new_node = BehaviorNode::Repeat {
                    times: 3,
                    children: Vec::new(),
                };
                let new_id = self.logic_state.add_node(parent, new_node);
                populate_treeview(self.logic_tree_view, &self.logic_state);
                self.handle_logic_tree_selchanged(new_id);
                set_text(self.logic_status_label, "Added 'Repeat' node.");
            }
            ID_LOGIC_ADD_IFELSE_BTN => {
                let parent = self.logic_selected_node_id;
                let new_node = BehaviorNode::IfElse {
                    condition: "target_detected".to_owned(),
                    then_branch: Vec::new(),
                    else_branch: Vec::new(),
                };
                let new_id = self.logic_state.add_node(parent, new_node);
                populate_treeview(self.logic_tree_view, &self.logic_state);
                self.handle_logic_tree_selchanged(new_id);
                set_text(self.logic_status_label, "Added 'If/Else' node.");
            }
            ID_LOGIC_DELETE_BTN => {
                if let Some(selected_id) = self.logic_selected_node_id {
                    self.logic_state.delete_node(selected_id);
                    self.logic_selected_node_id = None;
                    populate_treeview(self.logic_tree_view, &self.logic_state);
                    set_text(self.logic_node_type_label, "Node deleted.");
                    set_text(self.logic_prop1_edit, "");
                    set_text(self.logic_prop2_edit, "");
                    set_text(self.logic_prop3_edit, "");
                    set_text(
                        self.logic_status_label,
                        "Deleted selected node and subtree.",
                    );
                } else {
                    self.show_message("Select a node in the tree first.", MB_ICONWARNING);
                }
            }
            ID_LOGIC_APPLY_BTN => {
                self.apply_node_property_edits();
            }
            ID_LOGIC_SAVE_BTN => {
                self.export_logic_json(window);
            }
            ID_LOGIC_LOAD_BTN => {
                self.import_logic_json(window);
            }
            _ => {}
        }
    }

    fn read_edit_text(&self, edit: HWND) -> String {
        let mut buffer = [0u16; 256];
        let len = unsafe { GetWindowTextW(edit, &mut buffer) };
        if len > 0 {
            String::from_utf16_lossy(&buffer[..len as usize])
        } else {
            String::new()
        }
    }

    fn apply_node_property_edits(&mut self) {
        let Some(selected_id) = self.logic_selected_node_id else {
            self.show_message("Select a node in the tree first.", MB_ICONWARNING);
            return;
        };

        let prop1 = self.read_edit_text(self.logic_prop1_edit);
        let prop2 = self.read_edit_text(self.logic_prop2_edit);
        let prop3 = self.read_edit_text(self.logic_prop3_edit);

        let Some(item) = self.logic_state.get_item_mut(selected_id) else {
            return;
        };

        match &mut item.node {
            BehaviorNode::ClickImage {
                slot,
                confidence,
                delay_ms,
            } => {
                if let Ok(v) = prop1.trim().parse::<usize>() {
                    *slot = v.clamp(1, 99);
                }
                if let Ok(v) = prop2.trim().parse::<f32>() {
                    *confidence = v.clamp(0.1, 1.0);
                }
                if let Ok(v) = prop3.trim().parse::<u64>() {
                    *delay_ms = v.clamp(0, 60_000);
                }
            }
            BehaviorNode::Repeat { times, .. } => {
                if let Ok(v) = prop1.trim().parse::<u32>() {
                    *times = v;
                }
            }
            BehaviorNode::IfElse { condition, .. } => {
                *condition = prop1.trim().to_owned();
            }
            BehaviorNode::Sequence { name, .. } => {
                *name = prop1.trim().to_owned();
            }
        }

        populate_treeview(self.logic_tree_view, &self.logic_state);
        set_text(self.logic_status_label, "Applied node parameters.");
    }

    fn export_logic_json(&mut self, window: HWND) {
        let doc = self.logic_state.to_document();
        let json_text = match serde_json::to_string_pretty(&doc) {
            Ok(s) => s,
            Err(e) => {
                self.show_message(&format!("Failed to serialize logic: {e}"), MB_ICONERROR);
                return;
            }
        };

        let mut file_buffer = [0u16; 32_768];
        const FILTER: &[u16] = &[
            b'J' as u16,
            b'S' as u16,
            b'O' as u16,
            b'N' as u16,
            b' ' as u16,
            b'F' as u16,
            b'i' as u16,
            b'l' as u16,
            b'e' as u16,
            b's' as u16,
            0,
            b'*' as u16,
            b'.' as u16,
            b'j' as u16,
            b's' as u16,
            b'o' as u16,
            b'n' as u16,
            0,
            b'A' as u16,
            b'l' as u16,
            b'l' as u16,
            b' ' as u16,
            b'F' as u16,
            b'i' as u16,
            b'l' as u16,
            b'e' as u16,
            b's' as u16,
            0,
            b'*' as u16,
            b'.' as u16,
            b'*' as u16,
            0,
            0,
        ];
        let mut dialog = OPENFILENAMEW {
            lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
            hwndOwner: window,
            lpstrFilter: PCWSTR(FILTER.as_ptr()),
            lpstrFile: PWSTR(file_buffer.as_mut_ptr()),
            nMaxFile: file_buffer.len() as u32,
            Flags: OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST,
            ..Default::default()
        };

        let saved = unsafe { GetSaveFileNameW(&mut dialog).as_bool() };
        if !saved {
            return;
        }

        let end = file_buffer
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(file_buffer.len());
        let mut path = PathBuf::from(OsString::from_wide(&file_buffer[..end]));
        if path.extension().is_none() {
            path.set_extension("json");
        }

        if let Err(e) = fs::write(&path, json_text) {
            self.show_message(&format!("Could not save logic to file: {e}"), MB_ICONERROR);
        } else {
            set_text(
                self.logic_status_label,
                &format!(
                    "Exported logic tree to: {}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                ),
            );
        }
    }

    fn import_logic_json(&mut self, window: HWND) {
        let mut file_buffer = [0u16; 32_768];
        const FILTER: &[u16] = &[
            b'J' as u16,
            b'S' as u16,
            b'O' as u16,
            b'N' as u16,
            b' ' as u16,
            b'F' as u16,
            b'i' as u16,
            b'l' as u16,
            b'e' as u16,
            b's' as u16,
            0,
            b'*' as u16,
            b'.' as u16,
            b'j' as u16,
            b's' as u16,
            b'o' as u16,
            b'n' as u16,
            0,
            b'A' as u16,
            b'l' as u16,
            b'l' as u16,
            b' ' as u16,
            b'F' as u16,
            b'i' as u16,
            b'l' as u16,
            b'e' as u16,
            b's' as u16,
            0,
            b'*' as u16,
            b'.' as u16,
            b'*' as u16,
            0,
            0,
        ];
        let mut dialog = OPENFILENAMEW {
            lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
            hwndOwner: window,
            lpstrFilter: PCWSTR(FILTER.as_ptr()),
            lpstrFile: PWSTR(file_buffer.as_mut_ptr()),
            nMaxFile: file_buffer.len() as u32,
            Flags: OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST,
            ..Default::default()
        };

        let opened = unsafe { GetOpenFileNameW(&mut dialog).as_bool() };
        if !opened {
            return;
        }

        let end = file_buffer
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(file_buffer.len());
        let path = PathBuf::from(OsString::from_wide(&file_buffer[..end]));

        let contents = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                self.show_message(&format!("Could not read file: {e}"), MB_ICONERROR);
                return;
            }
        };

        let doc: BehaviorTreeDocument = match serde_json::from_str(&contents) {
            Ok(d) => d,
            Err(e) => {
                self.show_message(&format!("Invalid behavior tree JSON: {e}"), MB_ICONERROR);
                return;
            }
        };

        self.logic_state.load_from_document(&doc);
        self.logic_selected_node_id = None;
        populate_treeview(self.logic_tree_view, &self.logic_state);
        set_text(
            self.logic_status_label,
            &format!("Loaded tree '{}' successfully.", doc.name),
        );
    }
}
