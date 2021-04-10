use std::fs::File;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time;

use crate::FileType;

use rusqlite::{params, Connection, Result};
use walkdir::WalkDir;

const SCHEMA: &str = r#"
CREATE TABLE sqlar(
    name TEXT PRIMARY KEY,  -- name of the file
    mode INT,               -- access permissions
    mtime INT,              -- last modification time
    sz INT,                 -- original file size
    data BLOB               -- compressed content
);
"#;

/// Create a new archive and add all regular files and directories.
pub fn create(archive: impl AsRef<Path>, paths: &[impl AsRef<Path>]) -> Result<()> {
    let archive = archive.as_ref();
    if archive.exists() {
        eprintln!(
            "error: {} already exists. not creating a new one.",
            archive.display()
        );
        return Ok(());
    }

    let db = Connection::open(archive)?;
    crate::compress::init(&db)?;
    db.execute(SCHEMA, [])?;

    for path in paths {
        for entry in WalkDir::new(path.as_ref()) {
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
