use std::fs;

pub use rusqlite::{Connection, Result};

mod compress;
mod create;
mod extract;
mod list;

pub use extract::extract;
pub use list::with_each_file;
pub use create::create;

#[derive(Debug)]
pub struct Entry {
    pub name: String,
    pub mode: u32,
    pub filetype: FileType,
    pub mtime: i64,
    pub size: usize,
    pub compressed_size: usize,
    pub data: Option<Vec<u8>>,
}

#[derive(Debug, PartialEq)]
pub enum FileType {
    File,
    Dir,
    Unsupported,
}

// `stat.st_mode` values
// via https://man7.org/linux/man-pages/man7/inode.7.html
/// bit mask for the file type bit field
const S_IFMT: u32 = 0o0170000;
/// regular file
const S_IFREG: u32 = 0o0100000;
/// directory
const S_IFDIR: u32 = 0o0040000;

impl From<u32> for FileType {
    fn from(mode: u32) -> FileType {
        if mode & S_IFMT == S_IFREG {
            return FileType::File;
        }
        if mode & S_IFMT == S_IFDIR {
            return FileType::Dir;
        }

        FileType::Unsupported
    }
}

impl From<fs::FileType> for FileType {
    fn from(ft: fs::FileType) -> FileType {
        if ft.is_file() {
            return FileType::File;
        }
        if ft.is_dir() {
            return FileType::Dir;
        }
        FileType::Unsupported
    }
}
