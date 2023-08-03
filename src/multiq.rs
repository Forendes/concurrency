use std::sync::{Arc, Condvar, Mutex};
/// A lock-based general purpose queue. Implenemented based on the book
/// "C++ Concurrency in Action: Practical Multithreading" by Anthony Williams.
/// This queue uses 1 lock for head and 1 for tail, push() works on 1 lock and pop() uses 2 locks
/// if there is no data in head and it has to try to look in a tail.
#[derive(Debug, Clone)]
pub struct Multiq<T: Clone> {
    pub queue: Arc<InnerMultiq<T>>,
}

#[derive(Debug)]
pub struct InnerMultiq<T: Clone> {
    pub cvar: Condvar,
    pub head: Mutex<Data<T>>,
    pub tail: Mutex<Data<T>>,
}

#[derive(Debug, Clone)]
pub struct Data<T: Clone> {
    pub contents: (Option<T>, Option<Box<Data<T>>>),
}

impl<T: Clone> Data<T> {
    pub fn new(value: T) -> Data<T> {
        Data {
            contents: (Some(value), None),
        }
    }
}

impl<T: Clone + PartialEq + std::fmt::Debug> Multiq<T> {
    /// Creates a new queue.
    pub fn new(value: T) -> Multiq<T> {
        let queue = Data::new(value);
        let empty = Mutex::new(Data {
            contents: (None, None),
        });
        Multiq {
            queue: InnerMultiq {
                cvar: Condvar::new(),
                head: Mutex::new(queue),
                tail: empty,
            }
            .into(),
        }
    }

    /// Tales a value from the front of the queue.
    pub fn pop(&mut self) -> Option<T> {
        let head = &mut self
            .queue
            .head
            .lock()
            .expect("lock acquire failed")
            .contents;
        let mut value = None;
        if head.0.is_some() {
            value = head.0.clone();
            // shift head to next element
            if head.1.is_some() {
                *head = head.1.clone().unwrap().contents;
            } else {
                // try to add contents to head from tail
                let tail = &mut self
                    .queue
                    .tail
                    .lock()
                    .expect("lock acquire failed")
                    .contents;
                if tail.0.is_none() {
                    // nothing left
                    *head = (None, None);
                } else {
                    // load contents from tail and make tail empty
                    *head = tail.clone();
                }
                *tail = (None, None);
            }
        } else {
            // try to pop from tail
            let tail = &mut self
                .queue
                .tail
                .lock()
                .expect("lock acquire failed")
                .contents;
            if tail.0.is_some() {
                // pop from tail and load head from tail
                value = tail.0.clone();
                // update head if possible
                if tail.1.is_some() {
                    *head = tail.1.clone().unwrap().contents
                }
                // remove contents of tail
                *tail = (None, None);
            }
        }
        value
    }

    /// Pop that waits for a new value to be pushed into queue if it's empty.
    pub fn wait_and_pop(&mut self) -> T {
        let head = &mut self
            .queue
            .head
            .lock()
            .expect("lock acquire failed")
            .contents;
        let value;
        if head.0.is_some() {
            value = head.0.clone();
            // shift head to next element
            if head.1.is_some() {
                *head = head.1.clone().unwrap().contents;
            } else {
                // try to add contents to head from tail
                let tail = &mut self
                    .queue
                    .tail
                    .lock()
                    .expect("lock acquire failed")
                    .contents;
                if tail.0.is_none() {
                    // nothing left
                    *head = (None, None);
                } else {
                    // load contents from tail and make tail empty
                    *head = tail.clone();
                }
                *tail = (None, None);
            }
        } else {
            // try to pop from tail
            let mut tail_lock = self.queue.tail.lock().expect("lock acquire failed");
            if tail_lock.contents.0.is_some() {
                // pop from tail and load head from tail
                value = tail_lock.contents.0.clone();
                // update head if possible
                if tail_lock.contents.1.is_some() {
                    *head = tail_lock.contents.1.clone().unwrap().contents;
                }
            // wait for value to be pushed into tail
            } else {
                while tail_lock.contents.0.is_none() {
                    tail_lock = self.queue.cvar.wait(tail_lock).unwrap();
                }
                value = tail_lock.contents.0.clone();
            }
            // remove contents of tail
            tail_lock.contents = (None, None);
        }
        // always waits for value so can unwrap
        value.unwrap()
    }

    /// Pushes a value into the back of the queue.
    pub fn push(&mut self, value: T) {
        let mut tail_lock = self.queue.tail.lock().expect("lock acquire failed");
        if tail_lock.contents.0.is_none() {
            tail_lock.contents = (Some(value), None);
            drop(tail_lock);
        } else {
            let mut next = &mut tail_lock.contents.1;
            while next.is_some() {
                next = &mut next.as_mut().unwrap().contents.1;
            }
            *next = Some(Box::new(Data {
                contents: (Some(value), None),
            }));
        }
        self.queue.cvar.notify_one();
    }

    /// Returns true if the queue contains no elements.
    pub fn is_empty(&self) -> bool {
        let head = &self
            .queue
            .head
            .lock()
            .expect("lock acquire failed")
            .contents;
        let tail = &self
            .queue
            .tail
            .lock()
            .expect("lock acquire failed")
            .contents;
        tail.0.is_none() && tail.1.is_none() && head.0.is_none() && head.1.is_none()
    }
}
