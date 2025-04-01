use super::{N, *};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuccTable {
    pub entries: Vec<Option<String>>,
}

impl SuccTable {
    pub fn new() -> Self {
        SuccTable {
            entries: vec![None; *N],
        }
    }

    // Add this method to remove a specific successor
    pub fn remove_successor(&mut self, id: &str) {
        if let Some(index) = self.get_index(id) {
            let len = self.entries.len();
            // Shift remaining successors forward
            for i in index..len - 1 {
                self.entries[i] = self.entries[i + 1].clone();
            }
            // Set last entry to None
            self.entries[len - 1] = None;
        }
    }

    pub fn clear(&mut self) {
        for entry in self.entries.iter_mut() {
            *entry = None;
        }
    }

    pub fn insert_first(&mut self, id: String) {
        self.entries[0] = Some(id);
    }

    pub fn get_first(&self) -> Option<&String> {
        self.entries[0].as_ref()
    }
    pub fn get_index(&self, id: &str) -> Option<usize> {
        self.entries.iter().position(|entry| match entry {
            Some(s) => s == id,
            None => false,
        })
    }

    pub fn insert(&mut self, index: usize, id: Option<String>) {
        self.entries[index] = id;
    }
}
