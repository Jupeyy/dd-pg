use hiarc::Hiarc;

#[derive(Debug, Hiarc)]
pub struct StreamSample {
    pub data: Vec<u8>,
}
