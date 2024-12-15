use std::collections::HashMap;
use std::fmt;

pub struct CircularBuffer<T> {
    items: HashMap<usize, T>,
    head: usize,
    tail: usize,
    len: usize,
}

impl<T> CircularBuffer<T> {
    pub fn new() -> Self {
        CircularBuffer {
            items: HashMap::new(),
            head: 0,
            tail: 0,
            len: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn contains(&self, data: &T) -> bool
    where
        T: PartialEq,
    {
        self.items.values().any(|x| x == data)
    }

    pub fn push_back(&mut self, data: T) {
        if self.is_empty() {
            self.items.insert(0, data);
            self.head = 0;
            self.tail = 0;
        } else {
            self.tail = (self.tail + 1) % (self.len + 1);
            self.items.insert(self.tail, data);
        }
        self.len += 1;
    }

    #[allow(dead_code)]
    pub fn push_front(&mut self, data: T) {
        if self.is_empty() {
            self.items.insert(0, data);
            self.head = 0;
            self.tail = 0;
        } else {
            self.head = if self.head == 0 {
                self.len
            } else {
                self.head - 1
            };
            self.items.insert(self.head, data);
        }
        self.len += 1;
    }

    #[allow(dead_code)]
    pub fn pop_front(&mut self) -> Option<T> {
        if self.is_empty() {
            None
        } else {
            let item = self.items.remove(&self.head);
            self.len -= 1;
            if !self.is_empty() {
                self.head = (self.head + 1) % (self.len + 1);
            }
            item
        }
    }

    #[allow(dead_code)]
    pub fn pop_back(&mut self) -> Option<T> {
        if self.is_empty() {
            None
        } else {
            let item = self.items.remove(&self.tail);
            self.len -= 1;
            if !self.is_empty() {
                self.tail = if self.tail == 0 {
                    self.len
                } else {
                    self.tail - 1
                };
            }
            item
        }
    }

    #[allow(dead_code)]
    pub fn front(&self) -> Option<&T> {
        self.items.get(&self.head)
    }

    #[allow(dead_code)]
    pub fn back(&self) -> Option<&T> {
        self.items.get(&self.tail)
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        let len = self.len;
        let head = self.head;
        let items = &self.items;

        (0..len).map(move |i| {
            let actual_index = (head + i) % (len + 1);
            items.get(&actual_index).unwrap()
        })
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len {
            None
        } else {
            let actual_index = (self.head + index) % (self.len + 1);
            self.items.get(&actual_index)
        }
    }

    pub fn remove(&mut self, index: usize) -> Option<T> {
        if index >= self.len {
            None
        } else {
            let actual_index = (self.head + index) % (self.len + 1);
            let item = self.items.remove(&actual_index);

            // Shift all items after the removed one
            for i in actual_index..self.tail {
                if let Some(next_item) = self.items.remove(&(i + 1)) {
                    self.items.insert(i, next_item);
                }
            }

            self.len -= 1;
            if self.len > 0 {
                self.tail = if self.tail == 0 {
                    self.len
                } else {
                    self.tail - 1
                };
            }
            item
        }
    }
}

impl<T: fmt::Debug> fmt::Debug for CircularBuffer<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

// Implement Send and Sync for CircularBuffer if T is Send
unsafe impl<T: Send> Send for CircularBuffer<T> {}
unsafe impl<T: Sync> Sync for CircularBuffer<T> {}
