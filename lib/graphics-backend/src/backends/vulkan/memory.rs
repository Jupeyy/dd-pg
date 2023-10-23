use std::{
    collections::VecDeque,
    sync::{atomic::AtomicU64, Arc, Weak},
};

use super::{buffer::Buffer, mapped_memory::MappedMemory, memory_block::SDeviceMemoryBlock};

#[derive(Debug, Clone)]
pub struct SMemoryHeapQueueElement {
    pub allocation_size: usize,
    // only useful information for the heap
    offset_in_heap: usize,
    // useful for the user of this element
    pub offset_to_align: usize,
    pub element_in_heap: Option<Weak<spin::RwLock<SMemoryHeapElement>>>,
}

impl Default for SMemoryHeapQueueElement {
    fn default() -> Self {
        Self {
            allocation_size: Default::default(),
            offset_in_heap: Default::default(),
            offset_to_align: Default::default(),
            element_in_heap: None,
        }
    }
}

pub type TMemoryHeapQueue = std::collections::BTreeMap<usize, VecDeque<SMemoryHeapQueueElement>>;

#[derive(Debug, Clone)]
pub struct SMemoryHeapElement {
    allocation_size: usize,
    offset: usize,
    parent: Option<Weak<spin::RwLock<SMemoryHeapElement>>>,
    left: Option<Arc<spin::RwLock<SMemoryHeapElement>>>,
    right: Option<Arc<spin::RwLock<SMemoryHeapElement>>>,

    in_use: bool,
    // this is just to uniquely indentify the entry
    in_vec_id: usize,
}

impl Default for SMemoryHeapElement {
    fn default() -> Self {
        Self {
            allocation_size: Default::default(),
            offset: Default::default(),
            parent: None,
            left: None,
            right: None,
            in_use: Default::default(),
            in_vec_id: 0,
        }
    }
}

// some mix of queue and binary tree
#[derive(Debug, Clone)]
pub struct SMemoryHeap {
    root: Arc<spin::RwLock<SMemoryHeapElement>>,
    elements: TMemoryHeapQueue,
    in_vec_id: usize,
}

impl SMemoryHeap {
    pub fn new(size: usize, offset: usize) -> Arc<spin::Mutex<Self>> {
        let root = Arc::new(spin::RwLock::new(SMemoryHeapElement {
            allocation_size: size,
            offset: offset,
            parent: None,
            in_use: false,
            in_vec_id: 0,

            left: None,
            right: None,
        }));

        let mut queue_el = SMemoryHeapQueueElement::default();
        queue_el.allocation_size = size;
        queue_el.offset_in_heap = offset;
        queue_el.offset_to_align = offset;
        queue_el.element_in_heap = Some(Arc::downgrade(&root));

        let mut elements = TMemoryHeapQueue::default();
        let mut els = VecDeque::new();
        els.push_back(queue_el);
        elements.insert(size, els);

        Arc::new(spin::Mutex::new(Self {
            elements,
            root,
            in_vec_id: 0,
        }))
    }

    #[must_use]
    pub fn allocate(
        &mut self,
        alloc_size: usize,
        alloc_alignment: usize,
        allocated_memory: &mut SMemoryHeapQueueElement,
    ) -> bool {
        if self.elements.is_empty() {
            false
        } else {
            // calculate the alignment
            let elements = &mut self.elements;
            let mut first_entry = elements.first_entry().unwrap();
            let entry = first_entry.get().front().unwrap();
            let mut extra_size_align = entry.offset_in_heap % alloc_alignment;
            if extra_size_align != 0 {
                extra_size_align = alloc_alignment - extra_size_align;
            }
            let real_alloc_size = alloc_size + extra_size_align;

            // check if there is enough space in this instance
            if entry.allocation_size < real_alloc_size {
                false
            } else {
                let top_el = entry.clone();

                // remove entry from list
                first_entry.get_mut().pop_front();
                if first_entry.get().is_empty() {
                    // and in case the list is empty, remove the whole list
                    elements.remove(&top_el.allocation_size);
                }

                // make the heap entry in use, give the heap element a left child, which is this allocation
                let element_in_heap = top_el.element_in_heap.as_ref().unwrap().upgrade().unwrap();
                {
                    let mut element_in_heap = element_in_heap.write();
                    element_in_heap.in_use = true;

                    // the heap element gets children
                    let mut child_el = SMemoryHeapElement::default();
                    child_el.allocation_size = real_alloc_size;
                    child_el.offset = top_el.offset_in_heap;
                    child_el.parent = top_el.element_in_heap.clone();
                    child_el.in_use = true;
                    element_in_heap.left = Some(Arc::new(spin::RwLock::new(child_el)));
                }

                // in case the allocation was smaller, the heap element also gets a remaining child
                // which is not in use
                if real_alloc_size < top_el.allocation_size {
                    let mut remaining_el = SMemoryHeapQueueElement::default();
                    remaining_el.offset_in_heap = top_el.offset_in_heap + real_alloc_size;
                    remaining_el.allocation_size = top_el.allocation_size - real_alloc_size;

                    {
                        let mut child_el = SMemoryHeapElement::default();
                        child_el.allocation_size = remaining_el.allocation_size;
                        child_el.offset = remaining_el.offset_in_heap;
                        child_el.parent = top_el.element_in_heap.clone();
                        child_el.in_use = false;

                        self.in_vec_id += 1;
                        child_el.in_vec_id = self.in_vec_id;

                        let child_el = Arc::new(spin::RwLock::new(child_el));
                        element_in_heap.write().right = Some(child_el.clone());

                        remaining_el.element_in_heap = Some(Arc::downgrade(&child_el));

                        let key = remaining_el.allocation_size;
                        if elements.contains_key(&key) {
                            elements.get_mut(&key).unwrap().push_back(remaining_el);
                        } else {
                            let mut els = VecDeque::new();
                            els.push_back(remaining_el);
                            elements.insert(key, els);
                        }
                    }
                }

                // the result should know about the allocated memory
                allocated_memory.element_in_heap = element_in_heap
                    .read()
                    .left
                    .as_ref()
                    .map(|v| Arc::downgrade(v));

                allocated_memory.allocation_size = real_alloc_size;
                allocated_memory.offset_in_heap = top_el.offset_in_heap;
                allocated_memory.offset_to_align = top_el.offset_in_heap + extra_size_align;
                true
            }
        }
    }

    fn free(&mut self, allocated_memory: &SMemoryHeapQueueElement) {
        let mut continue_free = true;
        let mut this_el = allocated_memory.clone();
        while continue_free {
            // first check if the other block is in use, if not merge them again
            let this_heap_obj = this_el.element_in_heap.as_ref().unwrap().upgrade().unwrap();

            // parent of the heap memory to free
            let this_parent = this_heap_obj
                .read()
                .parent
                .as_ref()
                .map(|v| v.upgrade().unwrap());

            this_heap_obj.write().in_use = false;

            let mut other_heap_obj: Option<Arc<spin::RwLock<SMemoryHeapElement>>> = None;
            if let Some(this_parent) = &this_parent {
                if this_heap_obj.as_mut_ptr().eq(&this_parent
                    .read()
                    .left
                    .as_ref()
                    .map(|v| v.as_mut_ptr())
                    .unwrap_or(std::ptr::null_mut()))
                {
                    other_heap_obj = this_parent.read().right.clone();
                } else {
                    other_heap_obj = this_parent.read().left.clone();
                }
            }

            if (this_parent.is_some() && other_heap_obj.is_none())
                || (other_heap_obj.is_some() && !other_heap_obj.as_ref().unwrap().read().in_use)
            {
                // merge them
                if let Some(other_heap_obj) = &other_heap_obj {
                    let key = other_heap_obj.read().allocation_size;
                    let in_vec_id = other_heap_obj.read().in_vec_id;
                    let elements = &mut self.elements;
                    let vec = elements.get_mut(&key).unwrap();
                    vec.remove(
                        vec.iter()
                            .enumerate()
                            .find(|(_, v)| {
                                v.element_in_heap
                                    .as_ref()
                                    .unwrap()
                                    .upgrade()
                                    .unwrap()
                                    .read()
                                    .in_vec_id
                                    == in_vec_id
                            })
                            .unwrap()
                            .0,
                    );
                    // if list is empty, remove it
                    if vec.is_empty() {
                        elements.remove(&key);
                    }
                    other_heap_obj.write().in_use = false;
                }

                let mut parent_el = SMemoryHeapQueueElement::default();
                parent_el.offset_in_heap = this_parent.as_ref().unwrap().read().offset;
                parent_el.allocation_size = this_parent.as_ref().unwrap().read().allocation_size;
                parent_el.element_in_heap = this_parent.as_ref().map(|v| Arc::downgrade(v));

                this_parent.as_ref().unwrap().write().left = None;
                this_parent.as_ref().unwrap().write().right = None;

                this_el = parent_el;
            } else {
                // else just put this back into queue
                let key = this_el.allocation_size;
                self.in_vec_id += 1;
                this_heap_obj.write().in_vec_id = self.in_vec_id;
                let elements = &mut self.elements;
                if elements.contains_key(&key) {
                    elements.get_mut(&key).unwrap().push_back(this_el.clone());
                } else {
                    let mut els = VecDeque::new();
                    els.push_back(this_el.clone());
                    elements.insert(key, els);
                }
                continue_free = false;
            }
        }
    }

    #[must_use]
    pub fn is_used(&self) -> bool {
        return !self.root.read().in_use;
    }
}

#[derive(Debug)]
pub enum SMemoryHeapType<const ID: usize> {
    Cached(SMemoryHeapForVkMemory<ID>),
    None,
}

impl<const ID: usize> SMemoryHeapType<ID> {
    pub fn unwrap_ref(&self) -> &SMemoryHeapForVkMemory<ID> {
        match self {
            SMemoryHeapType::Cached(res) => res,
            SMemoryHeapType::None => panic!("memory was not part of a heap"),
        }
    }

    pub fn unwrap(self) -> SMemoryHeapForVkMemory<ID> {
        match self {
            SMemoryHeapType::Cached(res) => res,
            SMemoryHeapType::None => panic!("memory was not part of a heap"),
        }
    }
}

#[derive(Debug)]
pub struct SMemoryBlock<const ID: usize> {
    pub heap_data: SMemoryHeapQueueElement,

    pub used_size: AtomicU64,

    // optional
    pub buffer: Option<Arc<Buffer>>,

    pub buffer_mem: Arc<SDeviceMemoryBlock>,
    /// contains the offset too
    pub mapped_buffer: Option<(isize, Arc<MappedMemory>)>,

    pub heap: SMemoryHeapType<ID>,
}

impl<const ID: usize> SMemoryBlock<ID> {
    pub fn new(buffer_mem: Arc<SDeviceMemoryBlock>) -> Self {
        Self {
            heap_data: Default::default(),
            used_size: Default::default(),
            buffer: Default::default(),
            buffer_mem,
            mapped_buffer: None,
            heap: SMemoryHeapType::None,
        }
    }
}

impl<const ID: usize> Drop for SMemoryBlock<ID> {
    fn drop(&mut self) {
        match &mut self.heap {
            SMemoryHeapType::Cached(heap) => {
                let cleanups = heap.cleanups.upgrade().unwrap();

                let mut fake_this = SMemoryBlock::new(self.buffer_mem.clone());
                std::mem::swap(&mut fake_this, self);
                let mut heap = SMemoryHeapType::None;
                std::mem::swap(&mut fake_this.heap, &mut heap);

                let mut cleanups = cleanups.lock();

                let index = cleanups.cur_frame_index as usize;
                cleanups.cleanups[index].push((fake_this, heap.unwrap().heap));
            }
            SMemoryHeapType::None => {}
        }
    }
}

#[derive(Debug)]
pub struct SMemoryImageBlock<const ID: usize> {
    pub base: SMemoryBlock<ID>,
    pub image_memory_bits: u32,
}

#[derive(Debug, Clone)]
pub struct SMemoryHeapForVkMemory<const ID: usize> {
    pub heap: Arc<spin::Mutex<SMemoryHeap>>,
    pub cleanups: WeakFrameCleanupHeapBlocks<ID>,
    pub buffer: Option<Arc<Buffer>>,

    pub buffer_mem: Arc<SDeviceMemoryBlock>,
    pub mapped_buffer: Option<(isize, Arc<MappedMemory>)>,
}

impl<const ID: usize> SMemoryHeapForVkMemory<ID> {
    pub fn new(
        cleanups: WeakFrameCleanupHeapBlocks<ID>,
        buffer: Option<Arc<Buffer>>,
        buffer_mem: Arc<SDeviceMemoryBlock>,
        mapped_buffer: Option<(isize, Arc<MappedMemory>)>,
        size: usize,
        offset: usize,
    ) -> Self {
        Self {
            heap: SMemoryHeap::new(size, offset),
            cleanups,
            buffer,
            buffer_mem,
            mapped_buffer,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SMemoryCacheType<const ID: usize> {
    pub memory_heaps: Vec<SMemoryHeapForVkMemory<ID>>,
}

#[derive(Debug, Default)]
pub struct FrameCleanupHeapBlocks<const ID: usize> {
    pub cleanups: Vec<Vec<(SMemoryBlock<ID>, Arc<spin::Mutex<SMemoryHeap>>)>>,
    pub cur_frame_index: u32,
}
type ArcFrameCleanupHeapBlocks<const ID: usize> = Arc<spin::Mutex<FrameCleanupHeapBlocks<ID>>>;
type WeakFrameCleanupHeapBlocks<const ID: usize> = Weak<spin::Mutex<FrameCleanupHeapBlocks<ID>>>;

#[derive(Debug, Default)]
pub struct SMemoryBlockCache<const ID: usize> {
    pub memory_caches: SMemoryCacheType<ID>,
    pub frame_delayed_cached_buffer_cleanups: ArcFrameCleanupHeapBlocks<ID>,

    pub can_shrink: bool,
}

impl<const ID: usize> SMemoryBlockCache<ID> {
    pub fn init(&mut self, swap_chain_image_count: usize) {
        self.frame_delayed_cached_buffer_cleanups
            .lock()
            .cleanups
            .resize_with(swap_chain_image_count, || Default::default());
    }

    pub fn destroy_frame_data(&mut self, image_count: usize) {
        for i in 0..image_count {
            self.cleanup(i);
        }
        self.frame_delayed_cached_buffer_cleanups
            .lock()
            .cleanups
            .clear();
    }

    pub fn destroy(&mut self) {
        self.memory_caches.memory_heaps.clear();
        self.frame_delayed_cached_buffer_cleanups
            .lock()
            .cleanups
            .clear();
    }

    pub fn set_frame_index(&self, cur_frame_index: u32) {
        self.frame_delayed_cached_buffer_cleanups
            .lock()
            .cur_frame_index = cur_frame_index;
    }

    pub fn cleanup(&mut self, img_index: usize) {
        for (mem_block, heap) in
            &mut self.frame_delayed_cached_buffer_cleanups.lock().cleanups[img_index]
        {
            mem_block
                .used_size
                .store(0, std::sync::atomic::Ordering::SeqCst);
            heap.lock().free(&mem_block.heap_data);

            self.can_shrink = true;
        }
        self.frame_delayed_cached_buffer_cleanups.lock().cleanups[img_index].clear();
    }

    // returns the total free'd memory
    pub fn shrink(&mut self) -> usize {
        let mut freed_memory: usize = 0;
        if self.can_shrink {
            self.can_shrink = false;
            if self.memory_caches.memory_heaps.len() > 1 {
                let mut cur_size = self.memory_caches.memory_heaps.len();
                self.memory_caches.memory_heaps.retain_mut(|heap| {
                    if cur_size == 1 {
                        return true;
                    }
                    if heap.heap.lock().is_used() {
                        heap.mapped_buffer = None;
                        freed_memory += heap.buffer_mem.size as usize;

                        cur_size -= 1;
                        false
                    } else {
                        true
                    }
                })
            }
        }

        freed_memory
    }
}
