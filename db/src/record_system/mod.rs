use crate::file_system::{FileSystem, FS};

pub trait RecordSystem {
    fn create_database(name: &str);
    fn create_table(name: &str);
    fn use_database(name: &str);
    fn remove_database(name: &str);
    fn remove_table(name: &str);
}

pub struct RS {
    fs: FS,
    current_database: String,
}

impl RS {
    fn new() -> RS {
        RS {
            fs: FS::new(),
            current_database: String::new(),
        }
    }
}

impl RecordSystem for RS {
    fn create_database(name: &str) {}
    fn create_table(name: &str) {}
    fn use_database(name: &str) {}
    fn remove_database(name: &str) {}
    fn remove_table(name: &str) {}
}
