mod file_system;

use file_system::{FileSystem, FS};

fn main() {
    const PAGE_SIZE: usize = 256;

    let file = "file1";

    let mut fs = FS::new();
    println!("h1");
    fs.remove_file(file).unwrap_or(());
    fs.create_file(file).unwrap();
    println!("h2");
    let fd1 = fs.open_file(file).unwrap();
    println!("{}", fd1);
    let mut buf: [u8; PAGE_SIZE] = [1; PAGE_SIZE];

    for _round in 0..20 {
        for page in 0..140 {
            for i in 0..PAGE_SIZE {
                buf[i] = 97 + (i as u8) % 26;
            }
            fs.write_page(fd1, page, &buf).unwrap();
        }
    }

    println!("{}", fd1);
    println!("h7");
    fs.remove_file("file1").unwrap();
}
