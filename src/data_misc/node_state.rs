use crate::node::finger_table::finger_table::FingerTable;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NodeState {
    pub id: String,
    pub predecessor: Option<String>,
    pub successor: Option<String>,
    pub finger_table: FingerTable,
}

impl NodeState {
    pub fn new(id: String) -> Self {
        let finger_table = FingerTable::new(id.clone());
        #[cfg(debug_assertions)]
        println!("Finger table: {:?}", finger_table);
        NodeState {
            id: id.clone(),
            predecessor: Some(id.clone()),
            successor: Some(id.clone()),
            finger_table,
        }
    }
}
