//! Worker-local, bounded storage. No locks, shared ownership or unsafe code.
#![forbid(unsafe_code)]

/// A slot identifier that cannot accidentally address a recycled connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Key {
    index: usize,
    generation: u64,
}

struct Slot<T> {
    generation: u64,
    value: Option<T>,
}

/// Fixed-capacity generational storage with O(1) insertion and removal.
pub struct Slab<T> {
    slots: Vec<Slot<T>>,
    free: Vec<usize>,
    len: usize,
}

impl<T> Slab<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            slots: (0..capacity)
                .map(|_| Slot {
                    generation: 0,
                    value: None,
                })
                .collect(),
            free: (0..capacity).rev().collect(),
            len: 0,
        }
    }
    pub fn insert(&mut self, value: T) -> Result<Key, T> {
        let Some(index) = self.free.pop() else {
            return Err(value);
        };
        let slot = &mut self.slots[index];
        slot.value = Some(value);
        self.len += 1;
        Ok(Key {
            index,
            generation: slot.generation,
        })
    }
    pub fn get(&self, key: Key) -> Option<&T> {
        let slot = self.slots.get(key.index)?;
        (slot.generation == key.generation)
            .then_some(slot.value.as_ref())
            .flatten()
    }
    pub fn get_mut(&mut self, key: Key) -> Option<&mut T> {
        let slot = self.slots.get_mut(key.index)?;
        (slot.generation == key.generation)
            .then_some(slot.value.as_mut())
            .flatten()
    }
    pub fn remove(&mut self, key: Key) -> Option<T> {
        let slot = self.slots.get_mut(key.index)?;
        if slot.generation != key.generation {
            return None;
        }
        let value = slot.value.take()?;
        self.len -= 1;
        // Retire an exhausted generation instead of ever aliasing a stale key.
        if let Some(generation) = slot.generation.checked_add(1) {
            slot.generation = generation;
            self.free.push(key.index);
        }
        Some(value)
    }
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// A bounded cache of reusable byte buffers. Callers must bound live buffer growth.
/// Buffers larger than `buffer_size` are discarded on return.
pub struct BufferPool {
    buffers: Vec<Vec<u8>>,
    limit: usize,
    buffer_size: usize,
    available: usize,
}

impl BufferPool {
    pub fn new(limit: usize, buffer_size: usize) -> Self {
        Self {
            buffers: Vec::new(),
            limit,
            buffer_size,
            available: limit,
        }
    }
    pub fn take(&mut self) -> Option<Vec<u8>> {
        if self.available == 0 {
            return None;
        }
        self.available -= 1;
        Some(
            self.buffers
                .pop()
                .unwrap_or_else(|| Vec::with_capacity(self.buffer_size)),
        )
    }
    pub fn put(&mut self, mut buffer: Vec<u8>) {
        if self.available == self.limit {
            return;
        }
        self.available += 1;
        if buffer.capacity() <= self.buffer_size {
            buffer.clear();
            self.buffers.push(buffer);
        }
    }
    pub fn available(&self) -> usize {
        self.available
    }
}
