use crate::node::finger_table::finger_table::FingerTable;
use crate::node::succ_table::succ_table::SuccTable;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NodeState {
    pub id: String,
    pub predecessor: Option<String>,
    pub successor: SuccTable,
    pub finger_table: FingerTable,
}

impl NodeState {
    pub fn new(id: String) -> Self {
        let finger_table = FingerTable::new(id.clone());
        #[cfg(debug_assertions)]
        println!("Finger table: {:?}", finger_table);
        let mut successor = SuccTable::new();
        successor.insert_first(id.clone());
        NodeState {
            id: id.clone(),
            predecessor: Some(id.clone()),
            successor,
            finger_table,
        }
    }
}
