#[derive(Debug)]
pub enum DemoListEntry {
    File { name: String, date: String },
    Directory { name: String },
}

pub type DemoList = Vec<DemoListEntry>;
