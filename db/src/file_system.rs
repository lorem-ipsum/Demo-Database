extern crate nix;

use std::cell::RefCell;
use std::collections::HashMap;
use std::os::unix::io::RawFd;
use std::{fs::File, io, io::ErrorKind};

type PageID = i64;
const PAGE_SIZE: usize = 256;
type Page = [u8; PAGE_SIZE];
const BUF_SIZE: usize = 1024;

pub trait FileSystem {
    fn create_file(&mut self, name: &str) -> Result<(), io::Error>;
    fn open_file(&mut self, name: &str) -> Result<RawFd, nix::Error>;
    fn close_file(&mut self, fd: RawFd) -> Result<(), nix::Error>;
    fn remove_file(&mut self, name: &str) -> Result<(), io::Error>;
    fn read_page(
        &self,
        fd: RawFd,
        page: PageID,
        buf: &mut [u8; PAGE_SIZE],
    ) -> Result<usize, nix::Error>;
    fn write_page(
        &self,
        fd: RawFd,
        page: PageID,
        buf: &[u8; PAGE_SIZE],
    ) -> Result<usize, nix::Error>;
}

pub struct FS {
    name2raw: HashMap<String, Option<RawFd>>,
    raw2name: HashMap<RawFd, String>,
    buf_manager: RefCell<BufManager>,
}

struct BufManager {
    buf: [Page; BUF_SIZE],             // buffer content
    valid: [bool; BUF_SIZE],           // 1 for valid
    dirty: [bool; BUF_SIZE],           // 1 for dirty
    dest: [(RawFd, PageID); BUF_SIZE], // The corresponding memory
}

impl BufManager {
    ///
    ///  FD_FIELD=2     PAGE_FIELD=8
    /// ____________:____________________
    ///

    const FD_FIELD: usize = 2;
    const PAGE_FIELD: usize = 8;

    pub fn new() -> BufManager {
        BufManager {
            buf: [[0; PAGE_SIZE]; BUF_SIZE],
            valid: [false; BUF_SIZE],
            dirty: [false; BUF_SIZE],
            dest: [(0, 0); BUF_SIZE],
        }
    }

    ///
    /// Check if a page in memory is cached in the buffer.
    ///
    fn is_cached(&self, fd: RawFd, page: PageID) -> bool {
        let index = self._index(fd, page);
        self.valid[index] && self.dest[index] == (fd, page)
    }

    fn get(&self, fd: RawFd, page: PageID, buf: &mut [u8; PAGE_SIZE]) {
        let index = self._index(fd, page);
        buf.clone_from_slice(&self.buf[index]);
    }

    fn update(&mut self, fd: RawFd, page: PageID, buf: &[u8; PAGE_SIZE]) {
        let index = self._index(fd, page);
        self.buf[index].clone_from_slice(buf);
        self.dirty[index] = true;
    }

    fn _index(&self, fd: RawFd, page: PageID) -> usize {
        let fd1 = fd % (1 << BufManager::FD_FIELD);
        assert!(fd1 < 4);
        let page1 = page % (1 << BufManager::PAGE_FIELD);
        assert!(page1 < 256);

        let index: usize = (fd1 << BufManager::PAGE_FIELD) as usize + page1 as usize;
        assert!(index < 1024);

        index
    }
}

impl FS {
    pub fn new() -> FS {
        FS {
            name2raw: HashMap::new(),
            raw2name: HashMap::new(),
            buf_manager: RefCell::new(BufManager::new()),
        }
    }
    pub fn _print_info(&self) {
        println!("name2raw = {:?}", self.name2raw);
        println!("raw2name = {:?}", self.raw2name);
    }
}

impl FS {
    ///
    /// When closing a file, all cache pointing to pages in the file should be discarded.
    /// If the page is dirty, write back.
    ///
    fn _file_leave_cache(&self, fd: RawFd) {
        let fd1 = fd % (1 << BufManager::FD_FIELD);
        let start = (fd1 << BufManager::PAGE_FIELD) as usize;
        let end = ((fd1 + 1) << BufManager::PAGE_FIELD) as usize;
        for index in start..end {
            if self.buf_manager.borrow().valid[index]
                && self.buf_manager.borrow().dest[index].0 == fd
            {
                if self.buf_manager.borrow().dirty[index] {
                    // write back
                    self.write_page(
                        fd,
                        self.buf_manager.borrow().dest[index].1,
                        &self.buf_manager.borrow().buf[index],
                    )
                    .unwrap();
                }
                self.buf_manager.borrow_mut().valid[index] = false;
            }
        }
    }

    fn _create_buf(&self, fd: RawFd, page: PageID, buf: &[u8; PAGE_SIZE]) {
        let index = self.buf_manager.borrow()._index(fd, page);

        if self.buf_manager.borrow().valid[index] && self.buf_manager.borrow().dirty[index] {
            // write back the old cache
            self.write_page(
                self.buf_manager.borrow().dest[index].0,
                self.buf_manager.borrow().dest[index].1,
                &self.buf_manager.borrow().buf[index],
            )
            .unwrap();
        }

        self.buf_manager.borrow_mut().buf[index].clone_from_slice(buf);
        self.buf_manager.borrow_mut().valid[index] = true;
        self.buf_manager.borrow_mut().dirty[index] = false;
        self.buf_manager.borrow_mut().dest[index] = (fd, page);
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
                self._file_leave_cache(fd);
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
                    self._file_leave_cache(*fd);
                    self.raw2name.remove(fd);
                    self.name2raw.remove(name);
                    Ok(std::fs::remove_file(name)?)
                }
            }
        }
    }
    fn read_page(
        &self,
        fd: RawFd,
        page: PageID,
        buf: &mut [u8; PAGE_SIZE],
    ) -> Result<usize, nix::Error> {
        if self.buf_manager.borrow().is_cached(fd, page) {
            self.buf_manager.borrow().get(fd, page, buf);
            Ok(buf.len())
        } else {
            nix::unistd::lseek(fd, page, nix::unistd::Whence::SeekSet)?;
            nix::unistd::read(fd, buf)?;
            self._create_buf(fd, page, buf);
            Ok(buf.len())
        }
    }
    fn write_page(
        &self,
        fd: RawFd,
        page: PageID,
        buf: &[u8; PAGE_SIZE],
    ) -> Result<usize, nix::Error> {
        if self.buf_manager.borrow().is_cached(fd, page) {
            self.buf_manager.borrow_mut().update(fd, page, buf);
            Ok(buf.len())
        } else {
            nix::unistd::lseek(fd, page, nix::unistd::Whence::SeekSet)?;
            nix::unistd::write(fd, buf)?;
            self._create_buf(fd, page, buf);
            Ok(buf.len())
        }
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
