use ash::vk;

pub fn image_mip_level_count_ex(width: usize, height: usize, depth: usize) -> usize {
    (((std::cmp::max(width, std::cmp::max(height, depth)) as f32).log2()).floor() + 1.0) as usize
}

pub fn image_mip_level_count(img_extent: vk::Extent3D) -> usize {
    image_mip_level_count_ex(
        img_extent.width as usize,
        img_extent.height as usize,
        img_extent.depth as usize,
    )
}
