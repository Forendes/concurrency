use std::{
    alloc::{self, handle_alloc_error, Layout},
    fmt::Debug,
    mem::ManuallyDrop,
    ptr::{self, null_mut},
    sync::atomic::{AtomicPtr, AtomicUsize, Ordering},
};

type AllocatedNode<T> = ManuallyDrop<Nodus<T>>;

#[derive(Debug)]
pub struct Stackus<T> {
    pub head: AtomicPtr<AllocatedNode<T>>,
    pub threads_in_pop: AtomicUsize,
    pub list_to_delete: AtomicPtr<AllocatedNode<T>>,
}

#[derive(Debug)]
pub struct Nodus<T> {
    pub value: T,
    pub next: *mut AllocatedNode<T>,
}

impl<T> Stackus<T> {
    pub fn new(value: T) -> Self {
        let new_node = ManuallyDrop::new(Nodus {
            value,
            next: ptr::null_mut(),
        });
        let layout = Layout::new::<Nodus<T>>();
        let ptr = unsafe { alloc::alloc(layout) as *mut ManuallyDrop<Nodus<T>> };
        if ptr.is_null() {
            handle_alloc_error(layout);
        }
        unsafe { ptr::write(ptr, new_node) };
        Stackus {
            head: AtomicPtr::new(ptr),
            threads_in_pop: AtomicUsize::new(0),
            list_to_delete: AtomicPtr::new(null_mut()),
        }
    }

    pub fn push(&self, value: T) {
        let new_node = ManuallyDrop::new(Nodus {
            value,
            next: self.head.load(Ordering::SeqCst),
        });
        let layout = Layout::new::<Nodus<T>>();
        let ptr = unsafe { alloc::alloc(layout) as *mut ManuallyDrop<Nodus<T>> };
        if ptr.is_null() {
            handle_alloc_error(layout);
        }
        let heap_ref = unsafe {
            ptr::write(ptr, new_node);
            ptr.as_mut().expect("ptr is not null")
        };
        loop {
            match self.head.compare_exchange_weak(
                heap_ref.next,
                heap_ref,
                Ordering::SeqCst,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    break;
                }
                Err(_) => heap_ref.next = self.head.load(Ordering::SeqCst),
            }
        }
    }

    pub fn pop(&self) -> Option<T> {
        self.threads_in_pop.fetch_add(1, Ordering::SeqCst);
        let old_head = self.head.load(Ordering::SeqCst);
        loop {
            if !self.head.load(Ordering::SeqCst).is_null() {
                if self.head.compare_exchange_weak(
                    old_head,
                    unsafe { old_head.read().next },
                    Ordering::SeqCst,
                    Ordering::Relaxed,
                ).is_ok() {
                    let allocated_node = unsafe { old_head.read() };
                    let inner = ManuallyDrop::into_inner(allocated_node);
                    self.try_reclaim(old_head);
                    return Some(inner.value);
                }
            } else {
                self.threads_in_pop.fetch_sub(1, Ordering::SeqCst);
                return None;
            }
        }
    }

    fn try_reclaim(&self, old_head: *mut ManuallyDrop<Nodus<T>>) {
        if self.threads_in_pop.load(Ordering::SeqCst) == 1 {
            // claim list of nodes to be deleted
            let nodes_to_delete = self.list_to_delete.swap(ptr::null_mut(), Ordering::AcqRel);
            // check if counter is still 1 while list was creating and decrement so no other thread can access
            if self.threads_in_pop.fetch_sub(1, Ordering::SeqCst) == 1 {
                Self::delete_nodes(nodes_to_delete);
            } else {
                // if another pop started need to return back claimed nodes_to_delete
                self.chain_pending_nodes(nodes_to_delete);
            }
            // delete old_head
            unsafe { alloc::dealloc(old_head as _, Layout::for_value(&old_head.as_ref())) };
        } else {
            self.chain_pending_nodes(old_head);
            self.threads_in_pop.fetch_sub(1, Ordering::SeqCst);
        }
    }

    fn delete_nodes(list: *mut ManuallyDrop<Nodus<T>>) {
        while !list.is_null() {
            unsafe { alloc::dealloc(list as _, Layout::for_value(&list.as_ref())) };
        }
    }

    fn chain_pending_nodes(&self, list: *mut ManuallyDrop<Nodus<T>>) {
        let null = null_mut();
        // if list is null just insert else loop until next is null and insert taken list
        match self.list_to_delete.compare_exchange_weak(
            null,
            list,
            Ordering::SeqCst,
            Ordering::Relaxed,
        ) {
            Ok(_) => {}
            Err(_) => unsafe { self.list_to_delete.load(Ordering::SeqCst).read().next = list },
        }
    }
}

impl<T> Drop for Stackus<T> {
    fn drop(self: &mut Stackus<T>) {
        while self.pop().is_some() {}
    }
}
