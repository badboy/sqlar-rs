use rusqlite::{Connection, Result};

use crate::{Entry, FileType};

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
