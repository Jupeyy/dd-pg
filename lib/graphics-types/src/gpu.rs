use hiarc::Hiarc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Hiarc, Copy, Clone, Serialize, Deserialize)]
pub enum GpuType {
    Discrete = 0,
    Integrated,
    Virtual,
    Cpu,

    // should stay at last position in this enum
    Invalid,
}

#[derive(Debug, Hiarc, Clone, Serialize, Deserialize)]
pub struct Gpu {
    pub name: String,
    pub ty: GpuType,
}

#[derive(Debug, Hiarc, Clone, Serialize, Deserialize)]
pub struct CurGpu {
    pub name: String,
    pub msaa_sampling_count: u32,
}

#[derive(Debug, Hiarc, Clone, Serialize, Deserialize)]
pub struct Gpus {
    pub gpus: Vec<Gpu>,
    pub auto: Gpu,
    pub cur: CurGpu,
}
