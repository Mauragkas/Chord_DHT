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

    pub fn update_successors(&mut self, successors: Vec<Option<String>>) {
        for (i, successor) in successors.into_iter().take(*N).enumerate() {
            self.entries[i] = successor;
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

    pub fn get_last(&self) -> Option<&String> {
        self.entries[*N - 1].as_ref()
    }

    pub fn get_index(&self, id: &str) -> Option<usize> {
        self.entries.iter().position(|entry| match entry {
            Some(s) => s == id,
            None => false,
        })
    }

    pub fn push(&mut self, id: String) {
        let mut entries = self
            .entries
            .iter()
            .take(*N - 1)
            .map(|entry| entry.clone())
            .collect::<Vec<_>>();
        entries.insert(0, Some(id));
        self.entries = entries;
    }

    pub fn clear_after(&mut self, id: &str) {
        let index = self.get_index(id);
        if let Some(index) = index {
            for entry in self.entries.iter_mut().skip(index + 1) {
                *entry = None;
            }
        }
    }

    pub fn insert(&mut self, index: usize, id: Option<String>) {
        self.entries[index] = id;
    }

    pub fn insert_after(
        &mut self,
        id: String,
        id_to_insert: String,
    ) -> Result<usize, &'static str> {
        match self
            .entries
            .iter()
            .position(|entry| entry.as_ref() == Some(&id))
        {
            Some(index) if index + 1 < *N => {
                self.entries[index + 1] = Some(id_to_insert);
                Ok(index + 1)
            }
            Some(index) => Ok(index),
            None => {
                // If id not found, return error
                Err("ID not found in successor table")
            }
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Option<String>> {
        self.entries.iter()
    }
}
