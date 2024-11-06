use hiarc::Hiarc;

use crate::map::config::Config;

#[derive(Debug, Hiarc, Clone)]
pub struct ConfigSkeleton<C> {
    pub def: Config,
    pub user: C,
}

impl<C> From<ConfigSkeleton<C>> for Config {
    fn from(value: ConfigSkeleton<C>) -> Self {
        value.def
    }
}
