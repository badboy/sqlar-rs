use std::fs::{self, File};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::{list, FileType};

use filetime::FileTime;
use rusqlite::{Connection, Result};

/// Extract all files from the SQLar at `path` into `dest`
pub fn extract(path: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(&dest).expect("can't create target directory");

    let db = Connection::open(path)?;
    crate::compress::init(&db)?;

    list::with_each_file(&db, true, |entry| {
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
