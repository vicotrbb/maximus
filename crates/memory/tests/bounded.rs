use maximus_memory::{BufferPool, Slab};

#[test]
fn slab_bounds_and_stale_keys() {
    let mut slab = Slab::new(2);
    let a = slab.insert(10).unwrap();
    let b = slab.insert(20).unwrap();
    assert_eq!(slab.insert(30), Err(30));
    assert_eq!(slab.remove(a), Some(10));
    let c = slab.insert(40).unwrap();
    assert_ne!(a, c);
    assert_eq!(slab.get(a), None);
    assert_eq!(slab.remove(a), None);
    assert_eq!(slab.get(b), Some(&20));
    assert_eq!(slab.get(c), Some(&40));
    assert_eq!(slab.len(), 2);
}

#[test]
fn buffers_are_bounded_cleared_and_reused() {
    let mut pool = BufferPool::new(1, 32);
    let mut a = pool.take().unwrap();
    let ptr = a.as_ptr();
    assert!(pool.take().is_none());
    a.extend_from_slice(b"secret");
    pool.put(a);
    let b = pool.take().unwrap();
    assert!(b.is_empty());
    assert_eq!(b.as_ptr(), ptr);
    pool.put(b);
    assert_eq!(pool.available(), 1);
}

#[test]
fn empty_limits_deny_allocations() {
    assert!(Slab::<()>::new(0).insert(()).is_err());
    assert!(BufferPool::new(0, 32).take().is_none());
}
