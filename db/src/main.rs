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
    for i in 0..PAGE_SIZE {
        buf[i] = 97 + (i as u8) % 26;
    }
    println!("h4");
    let r = fs.write_page(fd1, 0, &buf).unwrap();
    println!("{}", fd1);
    println!("h5: {}", r);
    // println!("h5");
    // let mut buf2: [u8; PAGE_SIZE] = [0; PAGE_SIZE];
    // println!("h6");
    // fs.read_page(fd1, 0, &mut buf2).unwrap();
    println!("h7");
    fs.close_file(fd1).unwrap();

    // println!("buf2: {:?}", buf2);
}
