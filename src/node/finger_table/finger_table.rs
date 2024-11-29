use super::{hash::*, M, *};
use std::fmt::Debug;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FingerEntry {
    pub start: u32,
    pub id: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct FingerTable {
    pub entries: Vec<FingerEntry>,
}

impl Debug for FingerTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let entries: Vec<String> = self
            .entries
            .iter()
            .map(|entry| format!("{}:{}", entry.start, entry.id.as_deref().unwrap_or("None")))
            .collect();
        write!(f, "{:?}", entries)
    }
}

impl FingerTable {
    pub fn new(id: String) -> Self {
        let mut entries = Vec::with_capacity(*M);
        for i in 0..*M {
            // the start might wrap around the ring
            let start = (hash(&id) + 2u32.pow(i as u32)) % 2u32.pow(*M as u32);
            entries.push(FingerEntry { start, id: None });
        }
        FingerTable { entries }
    }

    pub fn update_entry(&mut self, index: usize, id: String) {
        self.entries[index].id = Some(id);
    }

    pub fn closest_preceding_finger(&self, id: u32) -> Option<String> {
        for i in (0..*M).rev() {
            if let Some(ref entry_id) = self.entries[i].id {
                if self.entries[i].start < id {
                    return Some(entry_id.clone());
                }
            }
        }
        None
    }

    pub async fn get_entry(&self, index: usize) -> String {
        self.entries[index].id.clone().expect("Entry ID is None")
    }
}
