use base::shared_index::{SharedIndex, SharedIndexCleanup};

pub type BufferObjectIndex = SharedIndex<dyn GraphicsBufferObjectHandleInterface>;

pub trait GraphicsBufferObjectHandleInterface: SharedIndexCleanup + std::fmt::Debug {
    fn create_buffer_object_slow(&mut self, upload_data: Vec<u8>) -> BufferObjectIndex;
    fn recreate_buffer_object_slow(
        &mut self,
        buffer_index: &BufferObjectIndex,
        upload_data: Vec<u8>,
    );
}
