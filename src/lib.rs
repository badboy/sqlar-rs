use std::fs::{self, File};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use filetime::FileTime;
pub use rusqlite::{Connection, Result};

mod extract;

/*
const SCHEMA: &str = r#"
CREATE TABLE sqlar(
    name TEXT PRIMARY KEY,  -- name of the file
    mode INT,               -- access permissions
    mtime INT,              -- last modification time
    sz INT,                 -- original file size
    data BLOB               -- compressed content
);
"#;
*/

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

#[derive(Debug)]
pub enum FileType {
    File,
    Dir,
    Unsupported,
}

impl From<u32> for FileType {
    fn from(mode: u32) -> FileType {
        match mode & !0o777 {
            0o100000 => FileType::File,
            0o40000 => FileType::Dir,
            _ => FileType::Unsupported,
        }
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

