#[derive(Debug, Clone)]
pub struct TextureContainer {
    pub width: usize,
    pub height: usize,
    pub depth: usize,
}

#[derive(Debug)]
pub struct GraphicsBufferObject {
    pub alloc_size: usize,
}
