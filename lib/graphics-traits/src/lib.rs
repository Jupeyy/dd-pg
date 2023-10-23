use graphics_base::streaming::GraphicsStreamHandleInterface;
use graphics_base_traits::traits::GraphicsSizeQuery;
use graphics_render_traits::GraphicsHandlesInterface;

pub trait GraphicsInterface:
    GraphicsSizeQuery + GraphicsStreamHandleInterface + GraphicsHandlesInterface
{
}
