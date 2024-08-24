use graphics_types::rendering::ColorRGBA;
use hiarc::Hiarc;
use math::math::vector::vec2;

// particles
#[derive(Debug, Hiarc, Clone)]
pub struct Particle {
    pub pos: vec2,
    pub vel: vec2,
    /// if the lifetime of this particle is above this value
    /// the velocity is not applied anymore
    pub max_lifetime_vel: f32,

    //int m_Spr;
    pub flow_affected: f32,

    pub life_span: f32,

    pub start_size: f32,
    pub end_size: f32,

    pub use_alpha_fading: bool,
    pub start_alpha: f32,
    pub end_alpha: f32,

    pub rot: f32,
    pub rot_speed: f32,

    pub gravity: f32,
    pub friction: f32,

    pub color: ColorRGBA,

    pub collides: bool,

    // set by the particle system
    pub life: f32,
    pub texture: &'static str,
}

impl Default for Particle {
    fn default() -> Self {
        Self {
            vel: vec2::new(0.0, 0.0),
            max_lifetime_vel: f32::MAX,
            life_span: 0.0,
            start_size: 1.0,
            end_size: 1.0,
            use_alpha_fading: false,
            start_alpha: 1.0,
            end_alpha: 1.0,
            rot: 0.0,
            rot_speed: 0.0,
            gravity: 0.0,
            friction: 0.0,
            flow_affected: 1.0,
            color: ColorRGBA {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
            collides: true,
            pos: vec2::new(0.0, 0.0),
            life: 0.0,
            texture: "invalid",
        }
    }
}
