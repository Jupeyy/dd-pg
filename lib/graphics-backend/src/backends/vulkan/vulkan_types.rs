use std::{
    collections::VecDeque,
    os::raw::c_void,
    sync::{Condvar, Mutex},
};

use ash::vk;
use graphics_types::command_buffer::AllCommands;
use num_derive::FromPrimitive;

pub const STAGING_BUFFER_CACHE_ID: usize = 0;
pub const STAGING_BUFFER_IMAGE_CACHE_ID: usize = 1;
pub const VERTEX_BUFFER_CACHE_ID: usize = 2;
pub const IMAGE_BUFFER_CACHE_ID: usize = 3;

/*
static void dbg_msg(const char *sys, const char *fmt, ...)
{
    va_list args;
    va_start(args, fmt);
    std::vprintf(fmt, args);
    va_end(args);
}
static constexpr const char *Localizable(const char *pStr) { return pStr; }

static void mem_copy(void *dest, const void *source, unsigned size) { memcpy(dest, source, size); }
static void dbg_break()
{
#ifdef __GNUC__
    __builtin_trap();
#else
    abort();
#endif
}
static void dbg_assert(int test, const char *msg)
{
    if(!test)
    {
        dbg_msg("assert", "%s", msg);
        dbg_break();
    }
}
static int str_comp(const char *a, const char *b) { return strcmp(a, b); }
static void str_copy(char *dst, const char *src, int dst_size)
{
    dst[0] = '\0';
    strncat(dst, src, dst_size - 1);
}
template<int N>
static void str_copy(char (&dst)[N], const char *src)
{
    str_copy(dst, src, N);
}
static int str_format(char *buffer, int buffer_size, const char *format, ...)
{
#if defined(CONF_FAMILY_WINDOWS)
    va_list ap;
    va_start(ap, format);
    _vsnprintf(buffer, buffer_size, format, ap);
    va_end(ap);

    buffer[buffer_size - 1] = 0; /* assure null termination */
#else
    va_list ap;
    va_start(ap, format);
    vsnprintf(buffer, buffer_size, format, ap);
    va_end(ap);

    /* null termination is assured by definition of vsnprintf */
#endif
    return 1;
}*/

/*
static_assert(std::chrono::steady_clock::is_steady, "Compiler does not support steady clocks, it might be out of date.");
static_assert(std::chrono::steady_clock::period::den / std::chrono::steady_clock::period::num >= 1000000000, "Compiler has a bad timer precision and might be out of date.");
static const std::chrono::time_point<std::chrono::steady_clock> tw_start_time = std::chrono::steady_clock::now();

int64_t time_get_impl() { return std::chrono::duration_cast<std::chrono::nanoseconds>(std::chrono::steady_clock::now() - tw_start_time).count(); }

std::chrono::nanoseconds time_get_nanoseconds() { return std::chrono::nanoseconds(time_get_impl()); }
-----  time ----- */

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum EMemoryBlockUsage {
    Texture = 0,
    Buffer,
    Stream,
    Staging,

    // whenever dummy is used, make sure to deallocate all memory
    Dummy,
}

/************************
 * STRUCT DEFINITIONS
 ************************/
#[derive(Debug, Clone)]
pub struct SDeviceMemoryBlock {
    pub mem: vk::DeviceMemory,
    pub size: vk::DeviceSize,
    pub usage_type: EMemoryBlockUsage,
}

impl Default for SDeviceMemoryBlock {
    fn default() -> Self {
        Self {
            mem: vk::DeviceMemory::null(),
            size: 0,
            usage_type: EMemoryBlockUsage::Dummy,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct SDeviceDescriptorPool {
    pub pool: vk::DescriptorPool,
    pub size: vk::DeviceSize,
    pub cur_size: vk::DeviceSize,
}

#[derive(Debug, Default, Clone)]
pub struct SDeviceDescriptorPools {
    pub pools: Vec<SDeviceDescriptorPool>,
    pub default_alloc_size: vk::DeviceSize,
    pub is_uniform_pool: bool,
}

#[derive(Debug, Clone)]
pub struct SDeviceDescriptorSet {
    pub descriptor: vk::DescriptorSet,
    pub pools: *const SDeviceDescriptorPools,
    pub pool_index: usize,
}

impl Default for SDeviceDescriptorSet {
    fn default() -> Self {
        Self {
            descriptor: Default::default(),
            pools: std::ptr::null(),
            pool_index: usize::MAX,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SMemoryHeapQueueElement {
    pub allocation_size: usize,
    // only useful information for the heap
    pub offset_in_heap: usize,
    // useful for the user of this element
    pub offset_to_align: usize,
    pub element_in_heap: *mut SMemoryHeapElement,
}

unsafe impl Send for SMemoryHeapQueueElement {}
unsafe impl Sync for SMemoryHeapQueueElement {}

impl Default for SMemoryHeapQueueElement {
    fn default() -> Self {
        Self {
            allocation_size: Default::default(),
            offset_in_heap: Default::default(),
            offset_to_align: Default::default(),
            element_in_heap: std::ptr::null_mut(),
        }
    }
}

pub type TMemoryHeapQueue = std::collections::BTreeMap<usize, VecDeque<SMemoryHeapQueueElement>>;

#[derive(Debug, Clone)]
pub struct SMemoryHeapElement {
    allocation_size: usize,
    offset: usize,
    parent: *mut SMemoryHeapElement,
    left: Option<Box<SMemoryHeapElement>>,
    right: Option<Box<SMemoryHeapElement>>,

    in_use: bool,
    in_vec_id: usize,
}

impl Default for SMemoryHeapElement {
    fn default() -> Self {
        Self {
            allocation_size: Default::default(),
            offset: Default::default(),
            parent: std::ptr::null_mut(),
            left: None,
            right: None,
            in_use: Default::default(),
            in_vec_id: 0,
        }
    }
}

// some mix of queue and binary tree
#[derive(Debug, Default, Clone)]
pub struct SMemoryHeap {
    root: SMemoryHeapElement,
    elements: TMemoryHeapQueue,
    in_vec_id: usize,
}

impl SMemoryHeap {
    pub fn init(&mut self, size: usize, offset: usize) {
        self.root.allocation_size = size;
        self.root.offset = offset;
        self.root.parent = std::ptr::null_mut();
        self.root.in_use = false;
        self.root.in_vec_id = 0;

        let mut queue_el = SMemoryHeapQueueElement::default();
        queue_el.allocation_size = size;
        queue_el.offset_in_heap = offset;
        queue_el.offset_to_align = offset;
        queue_el.element_in_heap = &mut self.root;
        if self.elements.contains_key(&size) {
            self.elements
                .get_mut(&size)
                .as_mut()
                .unwrap()
                .push_back(queue_el);
        } else {
            let mut els = VecDeque::new();
            els.push_back(queue_el);
            self.elements.insert(size, els);
        }

        self.in_vec_id = 0;
    }

    #[must_use]
    pub fn allocate(
        &mut self,
        alloc_size: usize,
        alloc_alignment: usize,
        allocated_memory: &mut SMemoryHeapQueueElement,
    ) -> bool {
        if self.elements.is_empty() {
            return false;
        } else {
            // calculate the alignment
            let mut first_entry = self.elements.first_entry().unwrap();
            let entry = first_entry.get().front().unwrap();
            let mut extra_size_align = entry.offset_in_heap % alloc_alignment;
            if extra_size_align != 0 {
                extra_size_align = alloc_alignment - extra_size_align;
            }
            let real_alloc_size = alloc_size + extra_size_align;

            // check if there is enough space in this instance
            if entry.allocation_size < real_alloc_size {
                return false;
            } else {
                let top_el = (*entry).clone();
                first_entry.get_mut().pop_front();
                if first_entry.get().is_empty() {
                    self.elements.remove(&top_el.allocation_size);
                }

                unsafe {
                    (*top_el.element_in_heap).in_use = true;

                    // the heap element gets children
                    (*top_el.element_in_heap).left =
                        Some(Box::<SMemoryHeapElement>::new(SMemoryHeapElement::default()));
                    (*(*top_el.element_in_heap).left.as_mut().unwrap()).allocation_size =
                        real_alloc_size;
                    (*(*top_el.element_in_heap).left.as_mut().unwrap()).offset =
                        top_el.offset_in_heap;
                    (*(*top_el.element_in_heap).left.as_mut().unwrap()).parent =
                        top_el.element_in_heap;
                    (*(*top_el.element_in_heap).left.as_mut().unwrap()).in_use = true;
                }

                if real_alloc_size < top_el.allocation_size {
                    let mut remaining_el = SMemoryHeapQueueElement::default();
                    remaining_el.offset_in_heap = top_el.offset_in_heap + real_alloc_size;
                    remaining_el.allocation_size = top_el.allocation_size - real_alloc_size;

                    unsafe {
                        (*top_el.element_in_heap).right =
                            Some(Box::<SMemoryHeapElement>::new(SMemoryHeapElement::default()));
                        (*top_el.element_in_heap)
                            .right
                            .as_mut()
                            .unwrap()
                            .allocation_size = remaining_el.allocation_size;
                        (*top_el.element_in_heap).right.as_mut().unwrap().offset =
                            remaining_el.offset_in_heap;
                        (*top_el.element_in_heap).right.as_mut().unwrap().parent =
                            top_el.element_in_heap;
                        (*top_el.element_in_heap).right.as_mut().unwrap().in_use = false;

                        remaining_el.element_in_heap =
                            (*top_el.element_in_heap).right.as_mut().unwrap().as_mut();

                        self.in_vec_id += 1;
                        (*remaining_el.element_in_heap).in_vec_id = self.in_vec_id;
                        let key = remaining_el.allocation_size;
                        if self.elements.contains_key(&key) {
                            self.elements.get_mut(&key).unwrap().push_back(remaining_el);
                        } else {
                            let mut els = VecDeque::new();
                            els.push_back(remaining_el);
                            self.elements.insert(key, els);
                        }
                    }
                }
                unsafe {
                    allocated_memory.element_in_heap =
                        (*top_el.element_in_heap).left.as_mut().unwrap().as_mut();
                }
                allocated_memory.allocation_size = real_alloc_size;
                allocated_memory.offset_in_heap = top_el.offset_in_heap;
                allocated_memory.offset_to_align = top_el.offset_in_heap + extra_size_align;
                return true;
            }
        }
    }

    pub fn free(&mut self, allocated_memory: &SMemoryHeapQueueElement) {
        let mut continue_free = true;
        let mut this_el = (*allocated_memory).clone();
        while continue_free {
            // first check if the other block is in use, if not merge them again
            let this_heap_obj = this_el.element_in_heap;
            unsafe {
                let this_parent = (*this_heap_obj).parent;
                (*this_heap_obj).in_use = false;
                let mut other_heap_obj: Option<&mut Box<SMemoryHeapElement>> = None;
                if !std::ptr::eq(this_parent, std::ptr::null())
                    && std::ptr::eq(
                        this_heap_obj,
                        (*this_parent).left.as_mut().unwrap().as_mut(),
                    )
                {
                    other_heap_obj = (*(*this_heap_obj).parent).right.as_mut();
                } else if !std::ptr::eq(this_parent, std::ptr::null()) {
                    other_heap_obj = (*(*this_heap_obj).parent).left.as_mut();
                }

                if (!std::ptr::eq(this_parent, std::ptr::null())
                    && other_heap_obj.as_ref().is_none())
                    || (other_heap_obj.as_ref().is_some())
                        && !(*other_heap_obj.as_ref().unwrap()).in_use
                {
                    // merge them
                    if other_heap_obj.as_ref().is_some() {
                        let key = (*other_heap_obj.as_ref().unwrap()).allocation_size;
                        let in_vec_id = (*other_heap_obj.as_ref().unwrap()).in_vec_id;
                        let vec = self.elements.get_mut(&key).unwrap();
                        vec.remove(
                            vec.iter()
                                .enumerate()
                                .find(|(_index, v)| (*v.element_in_heap).in_vec_id == in_vec_id)
                                .unwrap()
                                .0,
                        );
                        if vec.is_empty() {
                            self.elements.remove(&key);
                        }
                        (*other_heap_obj.unwrap()).in_use = false;
                    }

                    let mut parent_el = SMemoryHeapQueueElement::default();
                    parent_el.offset_in_heap = (*this_parent).offset;
                    parent_el.allocation_size = (*this_parent).allocation_size;
                    parent_el.element_in_heap = this_parent;

                    (*this_parent).left = None;
                    (*this_parent).right = None;

                    this_el = parent_el;
                } else {
                    // else just put this back into queue
                    let key = this_el.allocation_size;
                    self.in_vec_id += 1;
                    (*this_el.element_in_heap).in_vec_id = self.in_vec_id;
                    if self.elements.contains_key(&key) {
                        self.elements
                            .get_mut(&key)
                            .unwrap()
                            .push_back(this_el.clone());
                    } else {
                        let mut els = VecDeque::new();
                        els.push_back(this_el.clone());
                        self.elements.insert(key, els);
                    }
                    continue_free = false;
                }
            }
        }
    }

    #[must_use]
    pub fn is_used(&self) -> bool {
        return !self.root.in_use;
    }
}

#[derive(Debug, Clone)]
pub struct SMemoryBlock<const ID: usize> {
    pub heap_data: SMemoryHeapQueueElement,

    pub used_size: vk::DeviceSize,

    // optional
    pub buffer: vk::Buffer,

    pub buffer_mem: SDeviceMemoryBlock,
    pub mapped_buffer: *mut c_void,

    pub is_cached: bool,
    pub heap: *mut SMemoryHeap,
}

impl<const ID: usize> Default for SMemoryBlock<ID> {
    fn default() -> Self {
        Self {
            heap_data: Default::default(),
            used_size: Default::default(),
            buffer: Default::default(),
            buffer_mem: Default::default(),
            mapped_buffer: std::ptr::null_mut(),
            is_cached: Default::default(),
            heap: std::ptr::null_mut(),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct SMemoryImageBlock<const ID: usize> {
    pub base: SMemoryBlock<ID>,
    pub image_memory_bits: u32,
}

#[derive(Debug, Clone)]
pub struct SMemoryCacheHeap {
    pub heap: SMemoryHeap,
    pub buffer: vk::Buffer,

    pub buffer_mem: SDeviceMemoryBlock,
    pub mapped_buffer: *mut c_void,
}

impl Default for SMemoryCacheHeap {
    fn default() -> Self {
        Self {
            heap: Default::default(),
            buffer: Default::default(),
            buffer_mem: Default::default(),
            mapped_buffer: std::ptr::null_mut(),
        }
    }
}

unsafe impl Send for SMemoryCacheHeap {}
unsafe impl Sync for SMemoryCacheHeap {}

#[derive(Debug, Clone, Default)]
pub struct SMemoryCacheType {
    pub memory_heaps: Vec<Box<SMemoryCacheHeap>>,
}

#[derive(Debug, Clone, Default)]
pub struct SMemoryBlockCache<const ID: usize> {
    pub memory_caches: SMemoryCacheType,
    pub frame_delayed_cached_buffer_cleanups: Vec<Vec<SMemoryBlock<ID>>>,

    pub can_shrink: bool,
}

impl<const ID: usize> SMemoryBlockCache<ID> {
    pub fn init(&mut self, swap_chain_image_count: usize) {
        self.frame_delayed_cached_buffer_cleanups
            .resize(swap_chain_image_count, Default::default());
    }

    pub fn destroy_frame_data(&mut self, image_count: usize) {
        for i in 0..image_count {
            self.cleanup(i);
        }
        self.frame_delayed_cached_buffer_cleanups.clear();
    }

    pub fn destroy(&mut self, device: &ash::Device) {
        for (_index, heap_obj) in self.memory_caches.memory_heaps.iter_mut().enumerate() {
            let heap = heap_obj.as_mut();
            if !heap.mapped_buffer.is_null() {
                unsafe {
                    device.unmap_memory(heap.buffer_mem.mem);
                }
            }
            if heap.buffer != vk::Buffer::null() {
                unsafe {
                    device.destroy_buffer(heap.buffer, None);
                }
            }
            unsafe {
                device.free_memory(heap.buffer_mem.mem, None);
            }
        }

        self.memory_caches.memory_heaps.clear();
        self.frame_delayed_cached_buffer_cleanups.clear();
    }

    pub fn cleanup(&mut self, img_index: usize) {
        for mem_block in &mut self.frame_delayed_cached_buffer_cleanups[img_index] {
            mem_block.used_size = 0;
            unsafe { (*mem_block.heap).free(&mem_block.heap_data) };

            self.can_shrink = true;
        }
        self.frame_delayed_cached_buffer_cleanups[img_index].clear();
    }

    pub fn free_mem_block(&mut self, block: &SMemoryBlock<ID>, img_index: usize) {
        self.frame_delayed_cached_buffer_cleanups[img_index].push((*block).clone());
    }

    // returns the total free'd memory
    pub fn shrink(&mut self, device: &ash::Device) -> usize {
        let mut freed_memory: usize = 0;
        if self.can_shrink {
            self.can_shrink = false;
            if self.memory_caches.memory_heaps.len() > 1 {
                let mut cur_size = self.memory_caches.memory_heaps.len();
                self.memory_caches.memory_heaps.retain_mut(|heap| {
                    if cur_size == 1 {
                        return true;
                    }
                    let heap = heap.as_mut();
                    if heap.heap.is_used() {
                        unsafe {
                            if !heap.mapped_buffer.is_null() {
                                device.unmap_memory(heap.buffer_mem.mem);
                            }
                            if heap.buffer != vk::Buffer::null() {
                                device.destroy_buffer(heap.buffer, None);
                            }
                            device.free_memory(heap.buffer_mem.mem, None);
                        }
                        freed_memory += heap.buffer_mem.size as usize;

                        cur_size -= 1;
                        return false;
                    } else {
                        return true;
                    }
                })
            }
        }

        return freed_memory;
    }
}

#[derive(Debug, Clone)]
pub struct CTexture {
    pub img: vk::Image,
    pub img_mem: SMemoryImageBlock<IMAGE_BUFFER_CACHE_ID>,
    pub img_view: vk::ImageView,
    pub samplers: [vk::Sampler; 2],

    pub img_3d: vk::Image,
    pub img_3d_mem: SMemoryImageBlock<IMAGE_BUFFER_CACHE_ID>,
    pub img_3d_view: vk::ImageView,
    pub sampler_3d: vk::Sampler,

    pub width: usize,
    pub height: usize,
    pub depth: usize,
    pub rescale_count: u32,

    pub mip_map_count: u32,

    pub vk_standard_textured_descr_sets: [SDeviceDescriptorSet; 2],
    pub vk_standard_3d_textured_descr_set: SDeviceDescriptorSet,
    pub vk_text_descr_set: SDeviceDescriptorSet,
}

impl Default for CTexture {
    fn default() -> Self {
        Self {
            img: Default::default(),
            img_mem: Default::default(),
            img_view: Default::default(),
            samplers: Default::default(),
            img_3d: Default::default(),
            img_3d_mem: Default::default(),
            img_3d_view: Default::default(),
            sampler_3d: Default::default(),
            width: Default::default(),
            height: Default::default(),
            depth: Default::default(),
            rescale_count: Default::default(),
            mip_map_count: 1,
            vk_standard_textured_descr_sets: Default::default(),
            vk_standard_3d_textured_descr_set: Default::default(),
            vk_text_descr_set: Default::default(),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct SBufferObject {
    pub mem: SMemoryBlock<VERTEX_BUFFER_CACHE_ID>,
}

#[derive(Debug, Clone)]
pub struct SBufferObjectFrame {
    pub buffer_object: SBufferObject,

    pub cur_buffer: vk::Buffer,
    pub cur_buffer_offset: usize,
}

#[derive(Debug, Clone)]
pub struct SFrameBuffers {
    pub buffer: vk::Buffer,
    pub buffer_mem: SDeviceMemoryBlock,
    pub offset_in_buffer: usize,
    pub size: usize,
    pub used_size: usize,
    pub is_used: bool,
    pub mapped_buffer_data: *mut c_void,
}

impl Default for SFrameBuffers {
    fn default() -> Self {
        Self {
            buffer: Default::default(),
            buffer_mem: Default::default(),
            offset_in_buffer: Default::default(),
            size: Default::default(),
            used_size: Default::default(),
            is_used: Default::default(),
            mapped_buffer_data: std::ptr::null_mut(),
        }
    }
}

impl StreamMemory for SFrameBuffers {
    fn get_device_mem_block(&mut self) -> &mut SDeviceMemoryBlock {
        &mut self.buffer_mem
    }

    fn get_buffer(&mut self) -> &mut vk::Buffer {
        &mut self.buffer
    }

    fn get_offset_in_buffer(&mut self) -> &mut usize {
        &mut self.offset_in_buffer
    }

    fn get_used_size(&mut self) -> &mut usize {
        &mut self.used_size
    }

    fn get_is_used(&mut self) -> &mut bool {
        &mut self.is_used
    }

    fn get_size(&mut self) -> &mut usize {
        &mut self.size
    }

    fn get_mapped_buffer_data(&mut self) -> &mut *mut c_void {
        &mut self.mapped_buffer_data
    }

    fn get(&mut self) -> &mut SFrameBuffers {
        self
    }

    fn new(
        buffer: vk::Buffer,
        buffer_mem: SDeviceMemoryBlock,
        offset_in_buffer: usize,
        size: usize,
        used_size: usize,
        mapped_buffer_data: *mut c_void,
    ) -> Self {
        Self {
            buffer,
            buffer_mem,
            offset_in_buffer,
            size,
            used_size,
            is_used: Default::default(),
            mapped_buffer_data,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct SFrameUniformBuffers {
    pub base: SFrameBuffers,
    pub uniform_sets: [SDeviceDescriptorSet; 2],
}

impl StreamMemory for SFrameUniformBuffers {
    fn get_device_mem_block(&mut self) -> &mut SDeviceMemoryBlock {
        &mut self.base.buffer_mem
    }

    fn get_buffer(&mut self) -> &mut vk::Buffer {
        &mut self.base.buffer
    }

    fn get_offset_in_buffer(&mut self) -> &mut usize {
        &mut self.base.offset_in_buffer
    }

    fn get_used_size(&mut self) -> &mut usize {
        &mut self.base.used_size
    }

    fn get_is_used(&mut self) -> &mut bool {
        &mut self.base.is_used
    }

    fn get_size(&mut self) -> &mut usize {
        &mut self.base.size
    }

    fn get_mapped_buffer_data(&mut self) -> &mut *mut c_void {
        &mut self.base.mapped_buffer_data
    }

    fn get(&mut self) -> &mut SFrameBuffers {
        &mut self.base
    }

    fn new(
        buffer: vk::Buffer,
        buffer_mem: SDeviceMemoryBlock,
        offset_in_buffer: usize,
        size: usize,
        used_size: usize,
        mapped_buffer_data: *mut c_void,
    ) -> Self {
        Self {
            base: SFrameBuffers {
                buffer,
                buffer_mem,
                offset_in_buffer,
                size,
                used_size,
                is_used: Default::default(),
                mapped_buffer_data,
            },
            uniform_sets: Default::default(),
        }
    }
}

type TBufferObjectsOfFrame<TName> = Vec<TName>;
type TMemoryMapRangesOfFrame = Vec<vk::MappedMemoryRange>;

#[derive(Debug, Default, Clone)]
pub struct SStreamMemoryOfFrame<TName: Clone + Default> {
    pub buffer_objects_of_frames: TBufferObjectsOfFrame<TName>,
    pub buffer_objects_of_frames_range_datas: TMemoryMapRangesOfFrame,
    pub current_used_count: usize,
}

#[derive(Debug, Default, Clone)]
pub struct SStreamMemory<TName: Clone + Default + std::fmt::Debug> {
    stream_memory_of_frame: Vec<SStreamMemoryOfFrame<TName>>,
}

pub trait StreamMemory {
    fn get_device_mem_block(&mut self) -> &mut SDeviceMemoryBlock;
    fn get_buffer(&mut self) -> &mut vk::Buffer;
    fn get_offset_in_buffer(&mut self) -> &mut usize;
    fn get_used_size(&mut self) -> &mut usize;
    fn get_is_used(&mut self) -> &mut bool;
    fn reset_is_used(&mut self) {
        *self.get_used_size() = 0;
        *self.get_is_used() = false;
    }
    fn set_is_used(&mut self) {
        *self.get_is_used() = true;
    }
    fn get_size(&mut self) -> &mut usize;
    fn get_mapped_buffer_data(&mut self) -> &mut *mut c_void;
    fn get(&mut self) -> &mut SFrameBuffers;
    fn new(
        buffer: vk::Buffer,
        buffer_mem: SDeviceMemoryBlock,
        offset_in_buffer: usize,
        size: usize,
        used_size: usize,
        mapped_buffer_data: *mut c_void,
    ) -> Self;
}

impl<TName> SStreamMemory<TName>
where
    TName: Clone + StreamMemory + Default + std::fmt::Debug,
{
    pub fn get_buffers(&mut self, frame_image_index: usize) -> &mut Vec<TName> {
        return &mut self.stream_memory_of_frame[frame_image_index].buffer_objects_of_frames;
    }

    pub fn get_ranges(&mut self, frame_image_index: usize) -> &mut Vec<vk::MappedMemoryRange> {
        return &mut self.stream_memory_of_frame[frame_image_index]
            .buffer_objects_of_frames_range_datas;
    }

    pub fn get_buffers_and_ranges(
        &mut self,
        frame_image_index: usize,
    ) -> (&mut Vec<TName>, &mut Vec<vk::MappedMemoryRange>) {
        let stream_mem = &mut self.stream_memory_of_frame[frame_image_index];
        return (
            &mut stream_mem.buffer_objects_of_frames,
            &mut stream_mem.buffer_objects_of_frames_range_datas,
        );
    }

    pub fn get_used_count(&self, frame_image_index: usize) -> usize {
        return self.stream_memory_of_frame[frame_image_index].current_used_count;
    }

    pub fn increase_used_count(&mut self, frame_image_index: usize) {
        self.stream_memory_of_frame[frame_image_index].current_used_count += 1;
    }

    pub fn get_current_buffer(&mut self, frame_image_index: usize) -> &mut TName {
        let cur_count = self.stream_memory_of_frame[frame_image_index].current_used_count;
        return &mut self.stream_memory_of_frame[frame_image_index].buffer_objects_of_frames
            [cur_count - 1];
    }

    #[must_use]
    pub fn is_used(&self, frame_image_index: usize) -> bool {
        return self.get_used_count(frame_image_index) > 0;
    }

    pub fn reset_frame(&mut self, frame_image_index: usize) {
        self.stream_memory_of_frame[frame_image_index].current_used_count = 0;
    }

    pub fn init(&mut self, frame_image_count: usize) {
        self.stream_memory_of_frame
            .resize(frame_image_count, Default::default());
    }

    pub fn destroy<T>(&mut self, destroy_buffer: &mut T)
    where
        T: FnMut(usize, &mut TName),
    {
        let mut image_index: usize = 0;
        for memory_of_frame in &mut self.stream_memory_of_frame {
            let buffers_of_frame = &mut memory_of_frame.buffer_objects_of_frames;
            for i in 0..buffers_of_frame.len() {
                let buffer_of_frame = buffers_of_frame.get_mut(i).unwrap();
                let buffer_mem = buffer_of_frame.get_device_mem_block().mem;
                destroy_buffer(image_index, buffer_of_frame);

                // delete similar buffers
                for n in i..buffers_of_frame.len() {
                    let buffer_of_frame_del = buffers_of_frame.get_mut(n).unwrap();
                    if buffer_of_frame_del.get_device_mem_block().mem == buffer_mem {
                        *buffer_of_frame_del.get_buffer() = vk::Buffer::null();
                        buffer_of_frame_del.get_device_mem_block().mem = vk::DeviceMemory::null();
                    }
                }
            }
            image_index += 1;
        }
        self.stream_memory_of_frame.clear();
    }
}

pub struct SShaderModule {
    pub vert_shader_module: vk::ShaderModule,
    pub frag_shader_module: vk::ShaderModule,

    pub vk_device: ash::Device,
}

impl SShaderModule {
    pub fn new(vk_device: &ash::Device) -> Self {
        Self {
            vert_shader_module: Default::default(),
            frag_shader_module: Default::default(),
            vk_device: vk_device.clone(),
        }
    }
}

impl Drop for SShaderModule {
    fn drop(&mut self) {
        if self.vk_device.handle() != vk::Device::null() {
            if self.vert_shader_module != vk::ShaderModule::null() {
                unsafe {
                    self.vk_device
                        .destroy_shader_module(self.vert_shader_module, None);
                }
            }

            if self.frag_shader_module != vk::ShaderModule::null() {
                unsafe {
                    self.vk_device
                        .destroy_shader_module(self.frag_shader_module, None);
                }
            }
        }
    }
}

#[derive(FromPrimitive, Copy, Clone)]
#[repr(u32)]
pub enum EVulkanBackendAddressModes {
    Repeat = 0,
    ClampEdges = 1,

    Count = 2,
}

#[derive(FromPrimitive, Copy, Clone, PartialEq)]
#[repr(u32)]
pub enum EVulkanBackendBlendModes {
    Alpha = 0,
    None = 1,
    Additative = 2,

    Count = 3,
}

#[derive(FromPrimitive, Copy, Clone, PartialEq)]
#[repr(u32)]
pub enum EVulkanBackendClipModes {
    None = 0,
    DynamicScissorAndViewport = 1,

    Count = 2,
}

#[derive(PartialEq, Clone, Copy)]
pub enum EVulkanBackendTextureModes {
    NotTextured = 0,
    Textured,

    Count,
}

#[derive(Debug, Copy, Clone, FromPrimitive)]
pub enum RenderPassType {
    Single = 0,
    Dual,
}

pub const RENDER_PASS_TYPE_COUNT: usize = 2;
pub const MAX_SUB_PASS_COUNT: usize = 2;

#[derive(Debug, Default, Clone)]
pub struct SPipelineContainer {
    // 3 blend modes - 2 viewport & scissor modes - 2 texture modes
    pub pipeline_layouts: [[[[[vk::PipelineLayout; MAX_SUB_PASS_COUNT]; RENDER_PASS_TYPE_COUNT];
        EVulkanBackendTextureModes::Count as usize];
        EVulkanBackendClipModes::Count as usize];
        EVulkanBackendBlendModes::Count as usize],
    pub pipelines: [[[[[vk::Pipeline; MAX_SUB_PASS_COUNT]; RENDER_PASS_TYPE_COUNT];
        EVulkanBackendTextureModes::Count as usize];
        EVulkanBackendClipModes::Count as usize];
        EVulkanBackendBlendModes::Count as usize],
}

impl SPipelineContainer {
    pub fn destroy(&mut self, device: &ash::Device) {
        for pipe_layouts_list_list_list in &mut self.pipeline_layouts {
            for pipe_layouts_list_list in pipe_layouts_list_list_list {
                for pipe_layouts_list in pipe_layouts_list_list {
                    for pipe_layouts in pipe_layouts_list {
                        for pipe_layout in pipe_layouts {
                            if *pipe_layout != vk::PipelineLayout::null() {
                                unsafe {
                                    device.destroy_pipeline_layout(*pipe_layout, None);
                                }
                            }
                            *pipe_layout = vk::PipelineLayout::null();
                        }
                    }
                }
            }
        }
        for pipe_list_list_list in &mut self.pipelines {
            for pipe_list_list in pipe_list_list_list {
                for pipe_list in pipe_list_list {
                    for pipes in pipe_list {
                        for pipe in pipes {
                            if *pipe != vk::Pipeline::null() {
                                unsafe {
                                    device.destroy_pipeline(*pipe, None);
                                }
                            }
                            *pipe = vk::Pipeline::null();
                        }
                    }
                }
            }
        }
    }
}

pub enum ESupportedSamplerTypes {
    Repeat = 0,
    ClampToEdge,
    Texture2DArray,

    Count,
}

#[derive(Debug)]
pub struct SShaderFileCache {
    pub binary: Vec<u8>,
}

#[derive(Debug, Default)]
pub struct SSwapImgViewportExtent {
    pub swap_image_viewport: vk::Extent2D,
    pub has_forced_viewport: bool,
    pub forced_viewport: vk::Extent2D,
}

impl SSwapImgViewportExtent {
    // the viewport of the resulting presented image on the screen
    // if there is a forced viewport the resulting image is smaller
    // than the full swap image size
    pub fn get_presented_image_viewport(&self) -> vk::Extent2D {
        let mut viewport_width = self.swap_image_viewport.width;
        let mut viewport_height = self.swap_image_viewport.height;
        if self.has_forced_viewport {
            viewport_width = self.forced_viewport.width;
            viewport_height = self.forced_viewport.height;
        }

        return vk::Extent2D {
            width: viewport_width,
            height: viewport_height,
        };
    }
}

#[derive(Debug, Default, Clone)]
pub struct SwapChainImage {
    pub image: vk::Image,
    pub img_mem: SMemoryImageBlock<IMAGE_BUFFER_CACHE_ID>,
    pub img_view: vk::ImageView,
}

#[derive(Debug, Clone)]
pub struct VKDelayedBufferCleanupItem {
    pub buffer: vk::Buffer,
    pub mem: SDeviceMemoryBlock,
    pub mapped_data: *mut c_void,
}

impl Default for VKDelayedBufferCleanupItem {
    fn default() -> Self {
        Self {
            buffer: Default::default(),
            mem: Default::default(),
            mapped_data: std::ptr::null_mut(),
        }
    }
}

#[derive(Debug)]
pub struct SRenderThreadInner {
    pub is_rendering: bool,
    pub thread: Option<std::thread::JoinHandle<()>>,
    pub finished: bool,
    pub started: bool,
}

#[derive(Debug)]
pub struct SRenderThread {
    pub inner: Mutex<SRenderThreadInner>,
    pub cond: Condvar,
}

#[derive(Debug, Clone)]
pub struct SRenderCommandExecuteBuffer {
    pub raw_command: *const AllCommands,
    pub thread_index: usize,

    // must be calculated when the buffer gets filled
    pub estimated_render_call_count: usize,

    // useful data
    pub buffer: vk::Buffer,
    pub buffer_off: usize,
    pub descriptors: [SDeviceDescriptorSet; 2],

    pub index_buffer: vk::Buffer,

    pub clear_color_in_render_thread: bool,

    pub has_dynamic_state: bool,
    pub viewport: vk::Viewport,
    pub scissor: vk::Rect2D,

    pub render_pass_index: usize,
    pub sub_pass_index: usize,
}

impl Default for SRenderCommandExecuteBuffer {
    fn default() -> Self {
        Self {
            raw_command: std::ptr::null(),
            thread_index: Default::default(),
            estimated_render_call_count: Default::default(),
            buffer: Default::default(),
            buffer_off: Default::default(),
            descriptors: Default::default(),
            index_buffer: Default::default(),
            clear_color_in_render_thread: Default::default(),
            has_dynamic_state: Default::default(),
            viewport: Default::default(),
            scissor: Default::default(),
            render_pass_index: Default::default(),
            sub_pass_index: Default::default(),
        }
    }
}
