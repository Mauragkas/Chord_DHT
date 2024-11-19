use std::fmt;
use std::marker::PhantomData;
use std::ptr::NonNull;

pub struct Node<T> {
    data: T,
    next: NonNull<Node<T>>,
    prev: NonNull<Node<T>>,
}

pub struct CircularList<T> {
    head: Option<NonNull<Node<T>>>,
    len: usize,
    marker: PhantomData<Box<Node<T>>>,
}

// We need to implement Drop to prevent memory leaks
impl<T> Drop for CircularList<T> {
    fn drop(&mut self) {
        while self.pop_front().is_some() {}
    }
}

impl<T> Node<T> {
    fn new(data: T) -> NonNull<Self> {
        let node = Box::new(Self {
            data,
            next: NonNull::dangling(),
            prev: NonNull::dangling(),
        });
        let node_ptr = NonNull::new(Box::into_raw(node)).unwrap();
        unsafe {
            (*node_ptr.as_ptr()).next = node_ptr;
            (*node_ptr.as_ptr()).prev = node_ptr;
        }
        node_ptr
    }
}

impl<T> CircularList<T> {
    pub fn new() -> Self {
        CircularList {
            head: None,
            len: 0,
            marker: PhantomData,
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
        self.iter().any(|x| x == data)
    }

    pub fn push_back(&mut self, data: T) {
        let new_node = Node::new(data);
        self.len += 1;

        match self.head {
            None => {
                self.head = Some(new_node);
            }
            Some(head) => unsafe {
                let tail = (*head.as_ptr()).prev;
                (*new_node.as_ptr()).next = head;
                (*new_node.as_ptr()).prev = tail;
                (*head.as_ptr()).prev = new_node;
                (*tail.as_ptr()).next = new_node;
            },
        }
    }

    pub fn push_front(&mut self, data: T) {
        self.push_back(data);
        if self.len > 1 {
            unsafe {
                self.head = Some((*self.head.unwrap().as_ptr()).prev);
            }
        }
    }

    pub fn pop_front(&mut self) -> Option<T> {
        self.head.map(|head| unsafe {
            self.len -= 1;
            let old_head = Box::from_raw(head.as_ptr());
            let old_data = std::ptr::read(&old_head.data);

            if self.len == 0 {
                self.head = None;
            } else {
                let new_head = old_head.next;
                let tail = old_head.prev;
                (*new_head.as_ptr()).prev = tail;
                (*tail.as_ptr()).next = new_head;
                self.head = Some(new_head);
            }

            old_data
        })
    }

    pub fn pop_back(&mut self) -> Option<T> {
        if self.is_empty() {
            None
        } else {
            unsafe {
                let tail = (*self.head.unwrap().as_ptr()).prev;
                let old_tail = Box::from_raw(tail.as_ptr());
                let old_data = std::ptr::read(&old_tail.data);

                self.len -= 1;
                if self.len == 0 {
                    self.head = None;
                } else {
                    let new_tail = old_tail.prev;
                    let head = old_tail.next;
                    (*head.as_ptr()).prev = new_tail;
                    (*new_tail.as_ptr()).next = head;
                }

                Some(old_data)
            }
        }
    }

    pub fn front(&self) -> Option<&T> {
        unsafe { self.head.map(|node| &(*node.as_ptr()).data) }
    }

    pub fn back(&self) -> Option<&T> {
        unsafe {
            self.head
                .map(|head| &(*(*head.as_ptr()).prev.as_ptr()).data)
        }
    }

    pub fn rotate_left(&mut self) {
        if self.len > 1 {
            unsafe {
                self.head = Some((*self.head.unwrap().as_ptr()).next);
            }
        }
    }

    pub fn rotate_right(&mut self) {
        if self.len > 1 {
            unsafe {
                self.head = Some((*self.head.unwrap().as_ptr()).prev);
            }
        }
    }

    pub fn iter(&self) -> Iter<'_, T> {
        Iter {
            head: self.head,
            current: self.head,
            len: self.len,
            marker: PhantomData,
        }
    }

    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        IterMut {
            head: self.head,
            current: self.head,
            len: self.len,
            marker: PhantomData,
        }
    }
}

pub struct Iter<'a, T> {
    head: Option<NonNull<Node<T>>>,
    current: Option<NonNull<Node<T>>>,
    len: usize,
    marker: PhantomData<&'a Node<T>>,
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.len == 0 {
            None
        } else {
            self.len -= 1;
            self.current.map(|current| unsafe {
                let current_ref = &(*current.as_ptr()).data;
                self.current = Some((*current.as_ptr()).next);
                current_ref
            })
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

pub struct IterMut<'a, T> {
    head: Option<NonNull<Node<T>>>,
    current: Option<NonNull<Node<T>>>,
    len: usize,
    marker: PhantomData<&'a mut Node<T>>,
}

impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = &'a mut T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.len == 0 {
            None
        } else {
            self.len -= 1;
            self.current.map(|current| unsafe {
                let current_ptr = current.as_ptr();
                self.current = Some((*current_ptr).next);
                &mut (*current_ptr).data
            })
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl<T: fmt::Debug> fmt::Debug for CircularList<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

// Implement Send and Sync for CircularList if T is Send
unsafe impl<T: Send> Send for CircularList<T> {}
unsafe impl<T: Sync> Sync for CircularList<T> {}
