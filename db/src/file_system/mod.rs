extern crate nix;

use crate::common::{BUF_SIZE, FD_FIELD, PAGE_FIELD, PAGE_SIZE};
use std::cell::RefCell;
use std::collections::HashMap;
use std::os::unix::io::RawFd;
use std::{fs::File, io, io::ErrorKind};

type PageID = i64;
type Page = [u8; PAGE_SIZE];

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
        let fd1 = fd % (1 << FD_FIELD);
        assert!(fd1 < 4);
        let page1 = page % (1 << PAGE_FIELD);
        assert!(page1 < 256);

        let index: usize = (fd1 << PAGE_FIELD) as usize + page1 as usize;
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
        let fd1 = fd % (1 << FD_FIELD);
        let start = (fd1 << PAGE_FIELD) as usize;
        let end = ((fd1 + 1) << PAGE_FIELD) as usize;
        for index in start..end {
            if self.buf_manager.borrow().valid[index]
                && self.buf_manager.borrow().dest[index].0 == fd
            {
                if self.buf_manager.borrow().dirty[index] {
                    // write back
                    let page = self.buf_manager.borrow().dest[index].1.clone();
                    let buf = self.buf_manager.borrow().buf[index].clone();
                    self.write_page(fd, page, &buf).unwrap();
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
            nix::unistd::lseek(fd, page * PAGE_SIZE as i64, nix::unistd::Whence::SeekSet)?;
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
            nix::unistd::lseek(fd, page * PAGE_SIZE as i64, nix::unistd::Whence::SeekSet)?;
            nix::unistd::write(fd, buf)?;
            self._create_buf(fd, page, buf);
            Ok(buf.len())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::convert::TryInto;

    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[test]
    fn test_create_success() {
        let filename1 = "file1_test_create_success";
        let filename2 = "file2_test_create_success";
        let mut fs = FS::new();
        fs.create_file(filename1).unwrap();
        fs.create_file(filename2).unwrap();
        fs.remove_file(filename1).unwrap();
        fs.remove_file(filename2).unwrap();
    }

    #[test]
    fn test_create_failure() {
        let filename1 = "file1_test_create_failure";
        let filename2 = "file2_test_create_failure";
        let mut fs = FS::new();
        fs.create_file(filename1).unwrap();
        fs.create_file(filename2).unwrap();
        if fs.create_file(filename1).is_ok() {
            panic!("fuck");
        };
        fs.remove_file(filename1).unwrap();
        fs.remove_file(filename2).unwrap();
    }

    #[test]
    fn test_open_close_success() {
        let filename1 = "file1_test_open_close_success";
        let filename2 = "file2_test_open_close_success";

        let mut fs = FS::new();
        fs.create_file(filename1).unwrap();
        fs.create_file(filename2).unwrap();
        let fd1 = fs.open_file(filename1).unwrap();
        let fd2 = fs.open_file(filename2).unwrap();
        fs.close_file(fd1).unwrap();
        fs.close_file(fd2).unwrap();
        fs.remove_file(filename1).unwrap();
        fs.remove_file(filename2).unwrap();
    }

    #[test]
    fn test_double_opening() {
        let filename = "file_test_double_opening";
        let mut fs = FS::new();
        fs.create_file(filename).unwrap();
        fs.open_file(filename).unwrap();
        if fs.open_file(filename).is_ok() {
            panic!("fuck");
        }
        fs.remove_file(filename).unwrap();
    }

    #[test]
    fn test_double_closing() {
        let filename = "file_test_double_closing";
        let mut fs = FS::new();
        fs.create_file(filename).unwrap();
        let fd1 = fs.open_file(filename).unwrap();
        fs.close_file(fd1).unwrap();
        if fs.close_file(fd1).is_ok() {
            panic!("fuck");
        }
        fs.remove_file(filename).unwrap();
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
        let filename = "file_test_read_write_page";
        let mut fs = FS::new();
        fs.create_file(filename).unwrap();
        let fd1 = fs.open_file(filename).unwrap();

        let mut buf: [u8; PAGE_SIZE] = [1; PAGE_SIZE];
        for i in 0..PAGE_SIZE {
            buf[i] = 97 + (i as u8) % 26;
        }

        fs.write_page(fd1, 0, &buf).unwrap();
        let mut buf2: [u8; PAGE_SIZE] = [0; PAGE_SIZE];
        fs.read_page(fd1, 0, &mut buf2).unwrap();

        // remove file without closing first
        fs.remove_file(filename).unwrap();

        assert_eq!(buf, buf2);
    }

    #[test]
    fn test_100pages_write_read() {
        let filename = "file_test_100pages_write_read";
        let mut data: [u8; 100 * PAGE_SIZE] = [0; 100 * PAGE_SIZE];
        for i in 0..100 * PAGE_SIZE {
            data[i] = i as u8;
        }

        let mut fs = FS::new();
        fs.create_file(filename).unwrap();
        let fd1 = fs.open_file(filename).unwrap();

        for page in 0..100 {
            fs.write_page(
                fd1,
                page,
                &data[page as usize * PAGE_SIZE as usize..(page + 1) as usize * PAGE_SIZE as usize]
                    .try_into()
                    .unwrap(),
            )
            .unwrap();
        }

        fs.close_file(fd1).unwrap();

        let fd2 = fs.open_file(filename).unwrap();

        let mut buf2: [u8; PAGE_SIZE] = [1; PAGE_SIZE];

        for page in 0..100 {
            fs.read_page(fd2, page, &mut buf2).unwrap();

            assert_eq!(
                data[page as usize * PAGE_SIZE as usize..(page + 1) as usize * PAGE_SIZE as usize],
                buf2
            )
        }

        fs.remove_file(filename).unwrap();
    }
}
