mod file_system;

use file_system::{FileSystem, FS};

fn main() {
    println!("Hello, world!");
    let mut fs: FS = FS::new();

    fs.create_file("file1").unwrap();
    fs._print_info();
    let fd = fs.open_file("file1").unwrap();
    fs.close_file(fd).unwrap();
}
