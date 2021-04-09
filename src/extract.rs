//! Reimplementation of the `sqlar_compress` and `sqlar_uncompress` functions for SQLite.
//!
//! Not every sqlite build has these functions enabled, e.g. if Zlib wasn't available at build
//! time.
//! By reimplementing them we guarantee their availability.
//!
//! SQLite is Public Domain. See https://www.sqlite.org/copyright.html
//!
//! SPDX-FileCopyrightText: 2021 Jan-Erik Rediger <janerik@fnordig.de>
//! SPDX-License-Identifier: CC0-1.0

use rusqlite::functions::FunctionFlags;
use rusqlite::types::{Value, ValueRef};
use rusqlite::{Connection, Error, Result};

use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::io::prelude::*;

/// Create the functions `rusty_sqlar_compress` and `rusty_sqlar_uncompress` in the database.
pub fn init(db: &Connection) -> Result<()> {
    sqlar_compress(db)?;
    sqlar_uncompress(db)?;
    Ok(())
}

/// Implementation of the `sqlar_compress(X)` SQL function.
///
/// If the type of X is SQLITE_BLOB, and compressing that blob using
/// zlib utility function compress() yields a smaller blob, return the
/// compressed blob. Otherwise, return a copy of X.
///
/// SQLar uses the "zlib format" for compressed content.  The zlib format
/// contains a two-byte identification header and a four-byte checksum at
/// the end.  This is different from ZIP which uses the raw deflate format.
///
/// Future enhancements to SQLar might add support for new compression formats.
/// If so, those new formats will be identified by alternative headers in the
/// compressed data.
fn sqlar_compress(db: &Connection) -> Result<()> {
    db.create_scalar_function(
        "rusty_sqlar_compress",
        1,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
        |ctx| {
            assert_eq!(ctx.len(), 1, "called with unexpected number of arguments");

            let value = ctx.get_raw(0);
            let value = match value {
                // Try to compress a blob
                ValueRef::Blob(blob) => {
                    let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
                    enc.write_all(blob)
                        .map_err(|_| Error::UserFunctionError("error in compress()".into()))?;
                    match enc.finish() {
                        // If it is actually compressed, return the compressed data.
                        Ok(compressed) if compressed.len() < blob.len() => Value::Blob(compressed),
                        // Otherwise return a copy of the data
                        Ok(_) => Value::from(ValueRef::Blob(blob)),
                        // Or return an error
                        Err(_) => {
                            return Err(Error::UserFunctionError("error in compress()".into()))
                        }
                    }
                }
                // For other types return a copy
                value => Value::from(value),
            };

            Ok(value)
        },
    )
}

/// Implementation of the `sqlar_uncompress(X,SZ)` SQL function
///
/// Parameter SZ is interpreted as an integer. If it is less than or
/// equal to zero, then this function returns a copy of X. Or, if
/// SZ is equal to the size of X when interpreted as a blob, also
/// return a copy of X. Otherwise, decompress blob X using zlib
/// utility function uncompress() and return the results (another
/// blob).
fn sqlar_uncompress(db: &Connection) -> Result<()> {
    db.create_scalar_function(
        "rusty_sqlar_uncompress",
        2,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
        |ctx| {
            assert_eq!(ctx.len(), 2, "called with unexpected number of arguments");

            let value = ctx.get_raw(0);
            let size = ctx.get::<i32>(1)?;

            if size <= 0 {
                return Ok(Value::from(value));
            }

            let value = match value {
                // Already uncompressed, return a copy of the data
                ValueRef::Blob(blob) if blob.len() == size as usize => Value::from(value),
                // Otherwise, try to decode the blob.
                ValueRef::Blob(blob) => {
                    let mut dec = ZlibDecoder::new(blob);
                    let mut out = Vec::new();
                    dec.read_to_end(&mut out)
                        .map_err(|_| Error::UserFunctionError("error in compress()".into()))?;

                    Value::Blob(out)
                }
                // For other types return a copy
                value => Value::from(value),
            };

            Ok(value)
        },
    )
}
