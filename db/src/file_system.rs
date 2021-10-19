use std::{collections::HashSet, fs::File, io, io::ErrorKind};

pub fn print_message() {
    println!("fuck");
}

type PageID = u32;
trait FileSystem {
    fn create_file(self: &mut Self, name: &str) -> Result<(), io::Error>;
    fn open_file(self: &mut Self, name: &str) -> Result<(), io::Error>;
    fn close_file(self: &mut Self, name: &str) -> Result<(), io::Error>;
    fn remove_file(self: &mut Self, name: &str) -> Result<(), io::Error>;
    fn read_page(self: &mut Self, name: &str, page: PageID) -> Result<(), io::Error>;
    fn write_page(
        self: &mut Self,
        name: &str,
        page: PageID,
        content: &str,
    ) -> Result<(), io::Error>;
}

struct FS {
    file_names: HashSet<String>,
}

impl FileSystem for FS {
    fn create_file(self: &mut Self, name: &str) -> Result<(), io::Error> {
        if self.file_names.contains(name) {
            return Err(io::Error::new(
                ErrorKind::AlreadyExists,
                format!("Unable to create existing file {:?}.", name),
            ));
        } else {
            match File::create(&name) {
                Ok(f) => {
                    self.file_names.insert(name.to_owned());
                    return Ok(());
                }
                Err(e) => return Err(e),
            }
        }
    }
    fn open_file(self: &mut Self, name: &str) -> Result<(), io::Error> {
        Ok(())
    }
    fn close_file(self: &mut Self, name: &str) -> Result<(), io::Error> {
        Ok(())
    }
    fn remove_file(self: &mut Self, name: &str) -> Result<(), io::Error> {
        Ok(())
    }
    fn read_page(self: &mut Self, name: &str, page: PageID) -> Result<(), io::Error> {
        Ok(())
    }
    fn write_page(
        self: &mut Self,
        name: &str,
        page: PageID,
        content: &str,
    ) -> Result<(), io::Error> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[test]
    fn test_create_success() {
        let mut fs = FS {
            file_names: HashSet::<String>::new(),
        };
        fs.create_file("file1").unwrap();
        fs.create_file("file2").unwrap();
    }

    #[test]
    #[should_panic]
    fn test_create_failure() {
        let mut fs = FS {
            file_names: HashSet::<String>::new(),
        };
        fs.create_file("file1").unwrap();
        fs.create_file("file2").unwrap();
        if fs.create_file("file1").is_err() {
            panic!("fuck");
        };
    }
}
