use std::fs;

pub use rusqlite::Result;

mod compress;
mod create;
mod extract;
mod list;

pub use create::create;
pub use extract::extract;
pub use list::with_each_entry;

/// A file entry in the archive
#[derive(Debug, Clone)]
pub struct Entry {
    /// Name of the file
    pub name: String,
    /// Access permissions
    pub mode: u32,
    /// Either a file or directory.
    /// Other file types are unsupported and will not be created or extracted.
    pub filetype: FileType,
    /// Last modification time
    pub mtime: i64,
    /// Original file size
    pub size: usize,
    /// Compressed file size
    pub compressed_size: usize,
    /// Uncompressed content
    pub data: Option<Vec<u8>>,
}

/// A file's type
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum FileType {
    /// Regular file
    File,
    /// Directory
    Dir,
    /// Other file types are not supported
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
