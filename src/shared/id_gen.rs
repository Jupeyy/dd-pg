#[derive(PartialEq, Eq, Copy, Clone, Hash, bincode::Encode, bincode::Decode)]
pub struct IDGeneratorIDType(pub u64); // TODO! change visibility to private again

pub const ID_GENERATOR_ID_INVALID: IDGeneratorIDType = IDGeneratorIDType(0);
pub const ID_GENERATOR_ID_FIRST: IDGeneratorIDType = IDGeneratorIDType(1);

impl Default for IDGeneratorIDType {
    fn default() -> Self {
        ID_GENERATOR_ID_INVALID
    }
}

pub struct IDGenerator {
    cur_id: IDGeneratorIDType,
}

impl IDGenerator {
    pub fn new() -> Self {
        Self {
            cur_id: ID_GENERATOR_ID_FIRST,
        }
    }

    pub fn get_next(&mut self) -> IDGeneratorIDType {
        let cur = self.cur_id;
        self.cur_id.0 += 1;
        cur
    }

    pub fn is_valid(&self, id: &IDGeneratorIDType) -> bool {
        id.0 != ID_GENERATOR_ID_INVALID.0
    }
}
