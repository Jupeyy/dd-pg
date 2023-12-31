use std::{collections::HashMap, num::NonZeroUsize, sync::Arc};

use hashlink::LinkedHashMap;
use hiarc::HiArc;
use hiarc_macro::Hiarc;

use super::{
    buffer::Buffer, frame_resources::FrameResources, mapped_memory::MappedMemoryOffset,
    memory_block::DeviceMemoryBlock,
};

#[derive(Debug, Clone)]
pub struct MemoryHeapQueueElement {
    pub allocation_size: usize,
    // only useful information for the heap
    offset_in_heap: usize,
    // useful for the user of this element
    pub offset_to_align: usize,
    element_id: NonZeroUsize,
}

impl MemoryHeapQueueElement {
    pub fn new(element_id: NonZeroUsize) -> Self {
        Self {
            allocation_size: Default::default(),
            offset_in_heap: Default::default(),
            offset_to_align: Default::default(),
            element_id,
        }
    }
}

pub type TMemoryHeapQueue =
    std::collections::BTreeMap<usize, LinkedHashMap<NonZeroUsize, MemoryHeapQueueElement>>;

#[derive(Debug, Clone)]
pub struct MemoryHeapElement {
    allocation_size: usize,
    offset: usize,
    parent: Option<NonZeroUsize>,
    left: Option<NonZeroUsize>,
    right: Option<NonZeroUsize>,

    in_use: bool,
}

impl MemoryHeapElement {
    fn new() -> Self {
        Self {
            allocation_size: Default::default(),
            offset: Default::default(),
            parent: None,
            left: None,
            right: None,
            in_use: Default::default(),
        }
    }
}

// some mix of queue and binary tree
#[derive(Debug, Clone)]
pub struct MemoryHeap {
    queued_elements: TMemoryHeapQueue,

    root_id: NonZeroUsize,

    elements: HashMap<NonZeroUsize, MemoryHeapElement>,
    elements_id: NonZeroUsize,
}

impl MemoryHeap {
    pub fn new(size: usize, offset: usize) -> Self {
        let root = MemoryHeapElement {
            allocation_size: size,
            offset: offset,
            parent: None,
            in_use: false,

            left: None,
            right: None,
        };

        let root_id = NonZeroUsize::new(1).unwrap();

        let mut queue_el = MemoryHeapQueueElement::new(root_id);
        queue_el.allocation_size = size;
        queue_el.offset_in_heap = offset;
        queue_el.offset_to_align = offset;

        let mut queued_elements = TMemoryHeapQueue::default();
        let mut els = LinkedHashMap::new();
        els.insert(root_id, queue_el);
        queued_elements.insert(size, els);

        let mut elements = HashMap::new();
        elements.insert(root_id, root);

        Self {
            queued_elements,
            root_id,

            elements,
            elements_id: root_id,
        }
    }

    #[must_use]
    pub fn allocate(
        &mut self,
        alloc_size: usize,
        alloc_alignment: usize,
    ) -> Option<MemoryHeapQueueElement> {
        if self.queued_elements.is_empty() {
            None
        } else {
            // calculate the alignment
            let mut first_entry = self.queued_elements.first_entry().unwrap();
            let (_, entry) = first_entry.get().front().unwrap();
            let mut extra_size_align = entry.offset_in_heap % alloc_alignment;
            if extra_size_align != 0 {
                extra_size_align = alloc_alignment - extra_size_align;
            }
            let real_alloc_size = alloc_size + extra_size_align;

            // check if there is enough space in this instance
            if entry.allocation_size < real_alloc_size {
                None
            } else {
                let top_el = entry.clone();

                // remove entry from list
                first_entry.get_mut().pop_front();
                if first_entry.get().is_empty() {
                    // and in case the list is empty, remove the whole list
                    self.queued_elements.remove(&top_el.allocation_size);
                }

                // make the heap entry in use, give the heap element a left child, which is this allocation
                let element_in_heap = self.elements.get_mut(&top_el.element_id).unwrap();
                element_in_heap.in_use = true;

                // the heap element gets children
                let mut child_el = MemoryHeapElement::new();
                child_el.allocation_size = real_alloc_size;
                child_el.offset = top_el.offset_in_heap;
                child_el.parent = Some(top_el.element_id);
                child_el.in_use = true;
                self.elements_id = self.elements_id.checked_add(1).unwrap();
                let child_id = self.elements_id;
                element_in_heap.left = Some(child_id);

                self.elements.insert(child_id, child_el);

                // in case the allocation was smaller, the heap element also gets a remaining child
                // which is not in use
                if real_alloc_size < top_el.allocation_size {
                    self.elements_id = self.elements_id.checked_add(1).unwrap();
                    let remain_child_id = self.elements_id;
                    let mut remaining_el = MemoryHeapQueueElement::new(remain_child_id);
                    remaining_el.offset_in_heap = top_el.offset_in_heap + real_alloc_size;
                    remaining_el.allocation_size = top_el.allocation_size - real_alloc_size;

                    let mut child_el = MemoryHeapElement::new();
                    child_el.allocation_size = remaining_el.allocation_size;
                    child_el.offset = remaining_el.offset_in_heap;
                    child_el.parent = Some(top_el.element_id);
                    child_el.in_use = false;
                    self.elements.insert(remain_child_id, child_el);

                    let element_in_heap = self.elements.get_mut(&top_el.element_id).unwrap();
                    element_in_heap.right = Some(remain_child_id);

                    let key = remaining_el.allocation_size;
                    if self.queued_elements.contains_key(&key) {
                        self.queued_elements
                            .get_mut(&key)
                            .unwrap()
                            .insert(remain_child_id, remaining_el);
                    } else {
                        let mut els = LinkedHashMap::new();
                        els.insert(remain_child_id, remaining_el);
                        self.queued_elements.insert(key, els);
                    }
                }

                // the result should know about the allocated memory
                let mut allocated_memory = MemoryHeapQueueElement::new(child_id);

                allocated_memory.allocation_size = real_alloc_size;
                allocated_memory.offset_in_heap = top_el.offset_in_heap;
                allocated_memory.offset_to_align = top_el.offset_in_heap + extra_size_align;
                Some(allocated_memory)
            }
        }
    }

    fn free(&mut self, allocated_memory: &MemoryHeapQueueElement) {
        let mut continue_free = true;
        let mut this_el = allocated_memory.clone();
        while continue_free {
            // first check if the other block is in use, if not merge them again
            let this_heap_obj = self.elements.get_mut(&this_el.element_id).unwrap();
            this_heap_obj.in_use = false;

            // parent of the heap memory to free
            let this_parent_id = this_heap_obj.parent;

            let mut other_heap_obj_id: Option<NonZeroUsize> = None;
            if let Some(this_parent_id) = this_parent_id {
                let this_parent = self.elements.get_mut(&this_parent_id).unwrap();
                if Some(this_el.element_id) == this_parent.left {
                    other_heap_obj_id = this_parent.right;
                } else {
                    other_heap_obj_id = this_parent.left;
                }
            }

            if (this_parent_id.is_some() && other_heap_obj_id.is_none())
                || (other_heap_obj_id.is_some()
                    && !self
                        .elements
                        .get(&other_heap_obj_id.unwrap())
                        .unwrap()
                        .in_use)
            {
                // merge them
                if let Some(other_heap_obj_id) = &other_heap_obj_id {
                    let other_heap_obj = self.elements.get_mut(other_heap_obj_id).unwrap();
                    let key = other_heap_obj.allocation_size;
                    let queued_elements = &mut self.queued_elements;
                    let queue_map = queued_elements.get_mut(&key).unwrap();
                    queue_map.remove(other_heap_obj_id);
                    // if list is empty, remove it
                    if queue_map.is_empty() {
                        queued_elements.remove(&key);
                    }
                    other_heap_obj.in_use = false;
                }

                let this_parent_id = this_parent_id.unwrap();
                let mut parent_el = MemoryHeapQueueElement::new(this_parent_id);

                let this_parent = self.elements.get_mut(&this_parent_id).unwrap();
                parent_el.offset_in_heap = this_parent.offset;
                parent_el.allocation_size = this_parent.allocation_size;

                this_parent.left = None;
                this_parent.right = None;

                this_el = parent_el;
            } else {
                // else just put this back into queue
                let key = this_el.allocation_size;
                let queued_elements = &mut self.queued_elements;
                if queued_elements.contains_key(&key) {
                    queued_elements
                        .get_mut(&key)
                        .unwrap()
                        .insert(this_el.element_id, this_el.clone());
                } else {
                    let mut els = LinkedHashMap::new();
                    els.insert(this_el.element_id, this_el.clone());
                    queued_elements.insert(key, els);
                }
                continue_free = false;
            }
        }
    }

    #[must_use]
    pub fn is_used(&self) -> bool {
        self.elements.get(&self.root_id).unwrap().in_use
    }
}

#[derive(Debug)]
pub enum SMemoryHeapType {
    Cached {
        heap: HiArc<spin::Mutex<MemoryCache>>,
        id: usize,
    },
    None,
}

#[derive(Debug, Hiarc)]
pub struct MemoryBlock {
    pub heap_data: MemoryHeapQueueElement,

    // optional
    buffer: Option<HiArc<Buffer>>,

    buffer_mem: HiArc<DeviceMemoryBlock>,
    /// contains the offset too
    pub mapped_buffer: Option<MappedMemoryOffset>,

    heap: SMemoryHeapType,
}

impl MemoryBlock {
    pub fn new(
        heap_data: MemoryHeapQueueElement,
        buffer_mem: HiArc<DeviceMemoryBlock>,
        buffer: Option<HiArc<Buffer>>,
        mapped_buffer: Option<MappedMemoryOffset>,
        heap: SMemoryHeapType,
    ) -> HiArc<Self> {
        HiArc::new(Self {
            heap_data,
            buffer,
            buffer_mem,
            mapped_buffer,
            heap,
        })
    }

    pub fn buffer<'a>(
        self: &'a Arc<Self>,
        frame_resources: &mut FrameResources,
    ) -> &'a Option<HiArc<Buffer>> {
        frame_resources.memory_blocks.push(self.clone());

        &self.buffer
    }

    pub fn buffer_mem<'a>(
        self: &'a Arc<Self>,
        frame_resources: &mut FrameResources,
    ) -> &'a HiArc<DeviceMemoryBlock> {
        frame_resources.memory_blocks.push(self.clone());

        &self.buffer_mem
    }
}

impl Drop for MemoryBlock {
    fn drop(&mut self) {
        match &self.heap {
            SMemoryHeapType::Cached { heap, id } => {
                let mut heaps = heap.lock();
                let heap = heaps.memory_heaps.get_mut(id).unwrap();
                heap.heap.free(&self.heap_data);
                // shrink if possible
                if !heap.heap.is_used() {
                    heaps.memory_heaps.remove(&id);
                }
            }
            SMemoryHeapType::None => {}
        }
    }
}

#[derive(Debug)]
pub struct MemoryImageBlock {
    pub base: HiArc<MemoryBlock>,
    pub image_memory_bits: u32,
}

#[derive(Debug, Clone)]
pub struct MemoryHeapForVkMemory {
    pub heap: MemoryHeap,
    pub buffer: Option<HiArc<Buffer>>,

    pub buffer_mem: HiArc<DeviceMemoryBlock>,
    pub mapped_buffer: Option<MappedMemoryOffset>,
}

impl MemoryHeapForVkMemory {
    pub fn new(
        buffer: Option<HiArc<Buffer>>,
        buffer_mem: HiArc<DeviceMemoryBlock>,
        mapped_buffer: Option<MappedMemoryOffset>,
        size: usize,
        offset: usize,
    ) -> Self {
        Self {
            heap: MemoryHeap::new(size, offset),
            buffer,
            buffer_mem,
            mapped_buffer,
        }
    }
}

#[derive(Debug, Clone, Hiarc)]
pub struct MemoryCache {
    pub memory_heaps: LinkedHashMap<usize, MemoryHeapForVkMemory>,
    pub heap_id_gen: usize,
}

impl MemoryCache {
    pub fn new() -> HiArc<spin::Mutex<Self>> {
        HiArc::new(spin::Mutex::new(Self {
            heap_id_gen: 0,
            memory_heaps: Default::default(),
        }))
    }
}
