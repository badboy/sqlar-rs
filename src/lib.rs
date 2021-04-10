use std::fs::{self, File};
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time;

use filetime::FileTime;
use rusqlite::params;
pub use rusqlite::{Connection, Result};
use walkdir::WalkDir;

mod extract;

const SCHEMA: &str = r#"
CREATE TABLE sqlar(
    name TEXT PRIMARY KEY,  -- name of the file
    mode INT,               -- access permissions
    mtime INT,              -- last modification time
    sz INT,                 -- original file size
    data BLOB               -- compressed content
);
"#;

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

pub fn with_each_file(
    conn: &Connection,
    decompress: bool,
    mut f: impl FnMut(&Entry) -> Result<()>,
) -> Result<()> {
    let mut stmt = if decompress {
        conn.prepare(
            r#"
            SELECT
                name, mode, mtime, sz,
                rusty_sqlar_uncompress(data, sz) as data,
                COALESCE(LENGTH(data), 0) as compressed_size
            FROM
                sqlar
            "#,
        )?
    } else {
        conn.prepare("SELECT name, mode, mtime, sz,  COALESCE(LENGTH(data), 0) as compressed_size FROM sqlar")?
    };

    let entry_iter = stmt.query_map([], |row| {
        let raw_mode: u32 = row.get(1)?;
        let mode = raw_mode & 0o777;
        let filetype = FileType::from(raw_mode);

        Ok(Entry {
            name: row.get(0)?,
            mode,
            filetype,
            mtime: row.get(2)?,
            size: row.get(3)?,
            compressed_size: row.get(if decompress { 5 } else { 4 })?,
            data: if decompress { row.get(4)? } else { None },
        })
    })?;

    for entry in entry_iter {
        match entry {
            Ok(entry) => f(&entry)?,
            Err(e) => return Err(e),
        }
    }

    Ok(())
}

/// Extract all files from the SQLar at `path` into `dest`
pub fn extract(path: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(&dest).expect("can't create target directory");

    let db = Connection::open(path)?;
    extract::init(&db)?;

    with_each_file(&db, true, |entry| {
        if Path::new(&entry.name).is_absolute() {
            log::warn!("absolute file path found: {}, skipping.", entry.name);
            return Ok(());
        }

        let path = dest.join(&entry.name);

        match entry.filetype {
            FileType::Dir => {
                log::info!("Creating directory: {}", entry.name);
                fs::create_dir(&path).expect("can't create directory")
            }
            FileType::File => {
                log::info!("Creating file: {} (size: {})", entry.name, entry.size);
                let mut f = File::create(&path).expect("can't create file");

                if let Some(data) = &entry.data {
                    f.write_all(data).unwrap();
                }
            }
            FileType::Unsupported => {
                log::warn!("Unsupported file type for {}, skipping.", entry.name);
                return Ok(());
            }
        }

        let ft = FileTime::from_unix_time(entry.mtime, 0);
        filetime::set_file_mtime(&path, ft).unwrap();

        let attr = fs::metadata(&path).unwrap();
        let mut permissions = attr.permissions();
        permissions.set_mode(entry.mode);
        fs::set_permissions(&path, permissions).unwrap();

        Ok(())
    })?;

    Ok(())
}

/// Extract all files from the SQLar at `path` into `dest`
pub fn create(archive: &Path, paths: &[PathBuf]) -> Result<()> {
    if archive.exists() {
        eprintln!(
            "error: {} already exists. not creating a new one.",
            archive.display()
        );
        return Ok(());
    }

    let db = Connection::open(archive)?;
    extract::init(&db)?;
    db.execute(SCHEMA, [])?;

    for path in paths {
        for entry in WalkDir::new(path) {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    log::warn!("failed to read entry: {}", e);
                    continue;
                }
            };

            let path = &entry.path();
            let metadata = match entry.metadata() {
                Ok(metadata) => metadata,
                Err(e) => {
                    log::warn!("failed to read metadata for {}: {}", path.display(), e);
                    continue;
                }
            };

            let name = format!("{}", path.display());
            let file_type = FileType::from(entry.file_type());
            let modified = metadata.modified().unwrap_or(time::UNIX_EPOCH);
            let ts = modified
                .duration_since(time::UNIX_EPOCH)
                .map(|ts| ts.as_secs())
                .unwrap_or(0);
            let mode = metadata.permissions().mode();

            let data = if file_type == FileType::File {
                let mut f = match File::open(path) {
                    Ok(f) => f,
                    Err(e) => {
                        log::warn!("could not open file {}: {}", path.display(), e);
                        continue;
                    }
                };
                let mut data = Vec::with_capacity(metadata.len() as usize);
                if let Err(e) = f.read_to_end(&mut data) {
                    log::warn!("could not read file {}: {}", path.display(), e);
                    continue;
                }

                data
            } else {
                vec![]
            };

            let size = data.len();

            log::info!(
                "Adding: path={} | type={:?} | mode={:o} | mtime={}",
                path.display(),
                file_type,
                mode,
                ts
            );

            db.execute(
                r#"
                INSERT INTO
                    sqlar (name, mode, mtime, sz, data)
                VALUES
                    (?1, ?2, ?3, ?4, rusty_sqlar_compress(?5))
                "#,
                params![name, mode, ts, size, data],
            )?;
        }
    }

    Ok(())
}
