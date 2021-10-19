extern crate nix;

use std::collections::HashMap;
use std::os::unix::io::RawFd;
use std::{fs::File, io, io::ErrorKind};

type PageID = i64;
const PAGE_SIZE: usize = 4096;

pub trait FileSystem {
    fn create_file(&mut self, name: &str) -> Result<(), io::Error>;
    fn open_file(&mut self, name: &str) -> Result<RawFd, nix::Error>;
    fn close_file(&mut self, fd: RawFd) -> Result<(), nix::Error>;
    fn remove_file(&mut self, name: &str) -> Result<(), io::Error>;
    fn read_page(
        &mut self,
        fd: RawFd,
        page: PageID,
        buf: &mut [u8; PAGE_SIZE],
    ) -> Result<usize, nix::Error>;
    fn write_page(
        &mut self,
        fd: RawFd,
        page: PageID,
        buf: &[u8; PAGE_SIZE],
    ) -> Result<usize, nix::Error>;
}

pub struct FS {
    name2raw: HashMap<String, Option<RawFd>>,
    raw2name: HashMap<RawFd, String>,
}

impl FS {
    pub fn new() -> FS {
        FS {
            name2raw: HashMap::new(),
            raw2name: HashMap::new(),
        }
    }
    pub fn _print_info(&self) {
        println!("name2raw = {:?}", self.name2raw);
        println!("raw2name = {:?}", self.raw2name);
    }
}

impl FileSystem for FS {
    fn create_file(&mut self, name: &str) -> Result<(), io::Error> {
        if self.name2raw.contains_key(name) {
            Err(io::Error::new(
                ErrorKind::AlreadyExists,
                format!("Unable to create existing file {:?}.", name),
            ))
        } else {
            match File::create(&name) {
                Ok(_) => {
                    self.name2raw.insert(name.to_owned(), None);
                    Ok(())
                }
                Err(e) => Err(e),
            }
        }
    }
    fn open_file(&mut self, name: &str) -> Result<RawFd, nix::Error> {
        if !self.name2raw.contains_key(name) {
            Err(nix::Error::invalid_argument())
        } else {
            // println!("opening... {:?}", self.name2raw.get(name));
            match self.name2raw.get(name).unwrap() {
                None => {
                    // let f: RawFd = File::open(name)?.into_raw_fd();
                    let f: RawFd =
                        nix::fcntl::open(name, nix::fcntl::O_RDWR, nix::sys::stat::S_IXUSR)?;
                    *self.name2raw.get_mut(name).unwrap() = Some(f);
                    self.raw2name.insert(f, name.to_string());
                    Ok(f)
                }
                Some(_) => Err(nix::Error::invalid_argument()),
            }
        }
        // nix::fcntl::open("path", "oflag", "mode");
    }
    fn close_file(&mut self, fd: RawFd) -> Result<(), nix::Error> {
        match self.raw2name.get(&fd) {
            None => Err(nix::Error::invalid_argument()),
            Some(name) => {
                *self.name2raw.get_mut(name).unwrap() = None;
                self.raw2name.remove(&fd);
                nix::unistd::close(fd)
            }
        }
    }
    fn remove_file(&mut self, name: &str) -> Result<(), io::Error> {
        if !self.name2raw.contains_key(name) {
            // If the file doesn't exist.
            Err(io::Error::new(
                ErrorKind::NotFound,
                format!("Unable to find file {:?}.", name),
            ))
        } else {
            // The file exists, but may be open.
            match self.name2raw.get(name).unwrap() {
                None => {
                    self.name2raw.remove(name);
                    Ok(std::fs::remove_file(name)?)
                }
                Some(fd) => {
                    self.raw2name.remove(fd);
                    self.name2raw.remove(name);
                    Ok(std::fs::remove_file(name)?)
                }
            }
        }
    }
    fn read_page(
        &mut self,
        fd: RawFd,
        page: PageID,
        buf: &mut [u8; PAGE_SIZE],
    ) -> Result<usize, nix::Error> {
        nix::unistd::lseek(fd, page, nix::unistd::Whence::SeekSet)?;
        nix::unistd::read(fd, buf)
    }
    fn write_page(
        &mut self,
        fd: RawFd,
        page: PageID,
        buf: &[u8; PAGE_SIZE],
    ) -> Result<usize, nix::Error> {
        nix::unistd::lseek(fd, page, nix::unistd::Whence::SeekSet)?;
        nix::unistd::write(fd, buf)
    }
}

#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[test]
    fn test_create_success() {
        let mut fs = FS::new();
        fs.create_file("file1").unwrap();
        fs.create_file("file2").unwrap();
    }

    #[test]
    fn test_create_failure() {
        let mut fs = FS::new();
        fs.create_file("file1").unwrap();
        fs.create_file("file2").unwrap();
        if fs.create_file("file1").is_ok() {
            panic!("fuck");
        };
    }

    #[test]
    fn test_open_close_success() {
        let mut fs = FS::new();
        fs.create_file("file1").unwrap();
        fs.create_file("file2").unwrap();
        let fd1 = fs.open_file("file1").unwrap();
        let fd2 = fs.open_file("file2").unwrap();
        fs.close_file(fd1).unwrap();
        fs.close_file(fd2).unwrap();
    }

    #[test]
    fn test_double_opening() {
        let mut fs = FS::new();
        fs.create_file("file1").unwrap();
        fs.open_file("file1").unwrap();
        if fs.open_file("file1").is_ok() {
            panic!("fuck");
        }
    }

    #[test]
    fn test_double_closing() {
        let mut fs = FS::new();
        fs.create_file("file1").unwrap();
        let fd1 = fs.open_file("file1").unwrap();
        fs.close_file(fd1).unwrap();
        if fs.close_file(fd1).is_ok() {
            panic!("fuck");
        }
    }

    #[test]
    fn test_create_open_close_remove_recreate() {
        let mut fs = FS::new();
        fs.create_file("file1").unwrap();
        fs.create_file("file2").unwrap();
        fs.open_file("file1").unwrap();
        // remove a open file
        fs.remove_file("file1").unwrap();

        // remoe a closed file
        fs.remove_file("file2").unwrap();

        // recreate removed file
        fs.create_file("file1").unwrap();
        fs.create_file("file2").unwrap();

        // open recreated file
        let fd1 = fs.open_file("file1").unwrap();
        let fd2 = fs.open_file("file2").unwrap();
        fs.close_file(fd1).unwrap();
        fs.close_file(fd2).unwrap();
        fs.remove_file("file1").unwrap();
        fs.remove_file("file2").unwrap();
    }

    #[test]
    fn test_read_write_page() {
        let mut fs = FS::new();
        fs.create_file("file1").unwrap();
        let fd1 = fs.open_file("file1").unwrap();

        let mut buf: [u8; PAGE_SIZE] = [1; PAGE_SIZE];
        for i in 0..PAGE_SIZE {
            buf[i] = 97 + (i as u8) % 26;
        }

        fs.write_page(fd1, 0, &buf).unwrap();
        let mut buf2: [u8; PAGE_SIZE] = [0; PAGE_SIZE];
        fs.read_page(fd1, 0, &mut buf2).unwrap();

        fs.close_file(fd1).unwrap();

        // println!("buf2: {:?}", buf2);
        assert_eq!(buf, buf2);
    }
}
