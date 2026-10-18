use crate::gui::logic_gui::{BehaviorNode, BehaviorTreeDocument, LogicState};

#[test]
fn behavior_tree_document_round_trip_json() {
    let doc = BehaviorTreeDocument {
        name: "Test Routine".to_owned(),
        root: BehaviorNode::Sequence {
            name: "Main Flow".to_owned(),
            children: vec![
                BehaviorNode::ClickImage {
                    slot: 1,
                    confidence: 0.95,
                    delay_ms: 100,
                },
                BehaviorNode::Repeat {
                    times: 5,
                    children: vec![BehaviorNode::ClickImage {
                        slot: 2,
                        confidence: 0.88,
                        delay_ms: 50,
                    }],
                },
                BehaviorNode::IfElse {
                    condition: "Slot 3 Detected".to_owned(),
                    then_branch: vec![BehaviorNode::ClickImage {
                        slot: 4,
                        confidence: 0.92,
                        delay_ms: 200,
                    }],
                    else_branch: vec![BehaviorNode::ClickImage {
                        slot: 5,
                        confidence: 0.85,
                        delay_ms: 300,
                    }],
                },
            ],
        },
    };

    let json = serde_json::to_string_pretty(&doc).expect("Serialization failed");
    let deserialized: BehaviorTreeDocument =
        serde_json::from_str(&json).expect("Deserialization failed");

    assert_eq!(doc, deserialized);
}

#[test]
fn logic_state_tree_import_export_integrity() {
    let doc = BehaviorTreeDocument {
        name: "Nested Plan".to_owned(),
        root: BehaviorNode::Sequence {
            name: "Root Sequence".to_owned(),
            children: vec![
                BehaviorNode::ClickImage {
                    slot: 2,
                    confidence: 0.9,
                    delay_ms: 150,
                },
                BehaviorNode::Repeat {
                    times: 3,
                    children: vec![BehaviorNode::ClickImage {
                        slot: 3,
                        confidence: 0.85,
                        delay_ms: 80,
                    }],
                },
            ],
        },
    };

    let mut state = LogicState {
        next_id: 1,
        items: Vec::new(),
    };
    state.load_from_document(&doc);

    // Verify items populated
    assert_eq!(state.items.len(), 4);

    let exported = state.to_document();
    assert_eq!(exported.name, "Exported Behavior Tree");
    assert_eq!(exported.root, doc.root);
}

#[test]
fn logic_state_add_and_delete_node() {
    let mut state = LogicState {
        next_id: 1,
        items: Vec::new(),
    };
    let root_id = state.add_node(
        None,
        BehaviorNode::Sequence {
            name: "Root".to_owned(),
            children: Vec::new(),
        },
    );
    let child1 = state.add_node(
        Some(root_id),
        BehaviorNode::ClickImage {
            slot: 1,
            confidence: 0.9,
            delay_ms: 100,
        },
    );
    let repeat_id = state.add_node(
        Some(root_id),
        BehaviorNode::Repeat {
            times: 2,
            children: Vec::new(),
        },
    );
    let repeat_child = state.add_node(
        Some(repeat_id),
        BehaviorNode::ClickImage {
            slot: 2,
            confidence: 0.95,
            delay_ms: 50,
        },
    );

    assert_eq!(state.items.len(), 4);

    // Deleting repeat_id should delete both repeat and repeat_child
    state.delete_node(repeat_id);
    assert_eq!(state.items.len(), 2);
    assert!(state.get_item(repeat_id).is_none());
    assert!(state.get_item(repeat_child).is_none());
    assert!(state.get_item(child1).is_some());
    assert!(state.get_item(root_id).is_some());
}
