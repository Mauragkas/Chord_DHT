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

    pub fn clear(&mut self) {
        for entry in self.entries.iter_mut() {
            entry.id = None;
        }
    }

    pub fn get_first_entry(&self) -> u32 {
        self.entries[0].start
    }

    pub fn get_next_entry(&self, index: u32) -> Option<u32> {
        let current_pos = self.entries.iter().position(|entry| entry.start == index)?;
        let next_pos = current_pos + 1;
        if next_pos >= self.entries.len() {
            None
        } else {
            Some(self.entries[next_pos].start)
        }
    }

    pub fn update_entry(&mut self, index: u32, id: String) -> bool {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.start == index) {
            let changed = entry.id.as_ref() != Some(&id);
            entry.id = Some(id);
            changed
        } else {
            false
        }
    }
}
