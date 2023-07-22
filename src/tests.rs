use crate::multiq::Multiq;
use crate::stackus::Stackus;
use ::std::thread;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Barrier,
};
#[test]
fn queue_test() {
    let mut q = Multiq::new(1);
    let mut q2 = q.clone();
    let mut q3 = q.clone();
    let mut q4 = q.clone();
    let mut q5 = q.clone();
    let mut q6 = q.clone();

    let thread3 = thread::spawn(move || q2.push(2));
    thread3.join().unwrap();
    let thread1 = thread::spawn(move || {
        q3.pop();
    });
    let thread2 = thread::spawn(move || {
        q4.pop();
    });
    let thread3 = thread::spawn(move || {
        assert!(q5.wait_and_pop().is_some());
    });
    let thread4 = thread::spawn(move || q6.push(3));
    thread1.join().unwrap();
    thread2.join().unwrap();
    thread3.join().unwrap();
    thread4.join().unwrap();
    q.pop();
    assert_eq!(q.is_empty(), true);
}

#[test]
fn stack_push_works() {
    const THREAD_NUM: usize = 10;
    let mut handles = Vec::with_capacity(10);
    let stack = Arc::new(Stackus::new(0));
    let barrier = Arc::new(Barrier::new(THREAD_NUM));

    for _ in 0..THREAD_NUM {
        let barrier = barrier.clone();
        let stack = stack.clone();
        handles.push(thread::spawn(move || {
            barrier.wait();
            for i in 1..=5 {
                stack.push(i);
            }
        }));
    }
    for handle in handles {
        handle.join().unwrap();
    }
    let mut res = stack.pop();
    let mut sum_of_popped_values = 0 + res.unwrap();
    while res.is_some() {
        res = stack.pop();
        if res.is_some() {
            sum_of_popped_values = sum_of_popped_values + res.unwrap();
        }
    }
    assert_eq!(
        sum_of_popped_values,
        (1..=5).sum::<i32>() * THREAD_NUM as i32
    );
}

#[test]
fn stack_pop_works() {
    // sum of first 10 is 55
    let stack = Arc::new(Stackus::new(1));
    let mut i = 2;
    for _ in 0..9 {
        stack.push(i);
        i += 1;
    }
    const THREAD_NUM: usize = 5;
    let mut handles = Vec::with_capacity(5);
    let barrier = Arc::new(Barrier::new(THREAD_NUM));
    let results = Arc::new(AtomicUsize::new(0));

    for _ in 0..THREAD_NUM {
        let barrier = barrier.clone();
        let stack = stack.clone();
        let results1 = results.clone();
        let mut res = 0;
        handles.push(thread::spawn(move || {
            barrier.wait();
            while let Some(item) = stack.pop() {
                res = res + item;
            }
            results1.fetch_add(res, Ordering::Relaxed);
        }));
    }
    for handle in handles {
        handle.join().unwrap();
    }
    assert_eq!(55 as usize, results.load(Ordering::SeqCst));
}

#[test]
fn reclaim_works() {
    let arcus = Arc::new(1);
    let stack = Stackus::new(arcus.clone());
    while let Some(_) = stack.pop() {}
    assert_eq!(Arc::strong_count(&arcus), 1);
}
