use hiarc::Hiarc;

#[derive(Debug, Hiarc, Default, Clone)]
pub struct Config {
    // device props
    pub allows_linear_blitting: bool,
    pub optimal_swap_chain_image_blitting: bool,
    pub optimal_rgba_image_blitting: bool,
    pub linear_rgba_image_blitting: bool,
}
