use std::path::Path;

use rusqlite::{Connection, Result};

use crate::{Entry, FileType};

/// Iterate over each entry in the archive.
///
/// # Arguments
///
/// * `path` - path to the archive.
/// * `decompress` - wether to get and decompress the file data.
///                  If `false` no data is included.
/// * `f` - the function to run on each entry.
///
/// # Returns
///
/// `Ok(())` if iteration over all entries succeeds.
/// `Err(e)` if fetching entries fails, parsing entries fails
///  or the user-supplied callback fails.
pub fn with_each_entry(
    path: impl AsRef<Path>,
    decompress: bool,
    f: impl FnMut(&Entry) -> Result<()>,
) -> Result<()> {
    let db = Connection::open(path)?;
    iterate(&db, decompress, f)
}

/// Iterate over each entry in the archive.
///
/// # Arguments
///
/// * `conn` - the database to use
/// * `decompress` - wether to get and decompress the file data.
///                  If `false` no data is included.
/// * `f` - the function to run on each entry.
///
/// # Returns
///
/// `Ok(())` if iteration over all entries succeeds.
/// `Err(e)` if fetching entries fails, parsing entries fails
///  or the user-supplied callback fails.
pub(crate) fn iterate(
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
