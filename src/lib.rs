use std::fs;
use std::path::Path;

use rusqlite::Connection;
pub use rusqlite::Result;

mod compress;
mod create;
mod extract;
mod list;

pub use create::create;
pub use extract::extract;

/// A file entry in the archive
#[derive(Debug)]
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

#[derive(Debug, PartialEq)]
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

pub fn with_each_file(
    path: impl AsRef<Path>,
    decompress: bool,
    f: impl FnMut(&Entry) -> Result<()>,
) -> Result<()> {
    let db = Connection::open(path)?;
    list::with_each_file(&db, decompress, f)
}
