use std::fmt;

pub struct CircularBuffer<T> {
    items: Vec<T>,
}

impl<T> CircularBuffer<T> {
    pub fn new() -> Self {
        CircularBuffer { items: Vec::new() }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn contains(&self, data: &T) -> bool
    where
        T: PartialEq,
    {
        self.items.contains(data)
    }

    pub fn push_back(&mut self, data: T) {
        self.items.push(data);
    }

    #[allow(dead_code)]
    pub fn push_front(&mut self, data: T) {
        self.items.insert(0, data);
    }

    #[allow(dead_code)]
    pub fn pop_front(&mut self) -> Option<T> {
        if self.is_empty() {
            None
        } else {
            Some(self.items.remove(0))
        }
    }

    #[allow(dead_code)]
    pub fn pop_back(&mut self) -> Option<T> {
        self.items.pop()
    }

    #[allow(dead_code)]
    pub fn front(&self) -> Option<&T> {
        self.items.first()
    }

    #[allow(dead_code)]
    pub fn back(&self) -> Option<&T> {
        self.items.last()
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.items.iter()
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        self.items.get(index)
    }

    pub fn remove(&mut self, index: usize) -> Option<T> {
        if index >= self.len() {
            None
        } else {
            Some(self.items.remove(index))
        }
    }

    pub fn rotate(&mut self) {
        if self.len() <= 1 {
            return;
        }
        let item = self.items.remove(0);
        self.items.push(item);
    }
}

impl<T: fmt::Debug> fmt::Debug for CircularBuffer<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

unsafe impl<T: Send> Send for CircularBuffer<T> {}
unsafe impl<T: Sync> Sync for CircularBuffer<T> {}
