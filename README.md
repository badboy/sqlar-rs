# sqlar

[![Crates.io version](https://img.shields.io/crates/v/sqlar.svg?style=flat-square)](https://crates.io/crates/sqlar)
[![docs.rs docs](https://img.shields.io/badge/docs-latest-blue.svg?style=flat-square)](https://docs.rs/sqlar)
[![License: MIT](https://img.shields.io/github/license/badboy/sqlar-rs?style=flat-square)](LICENSE)

## sqlar - a SQLite Archive utility

> An "SQLite Archive" is a file container similar to a ZIP archive or Tarball but based on an SQLite database.

See the [SQLite Archive Files][sqlar] documentation for all information.

This library allows to list archive contents, extract files from archives or create a new
archive.
It's main usage is throug the command line utility `sqlar`.

## Installation

The command line utility `sqlar` can be installed through `cargo`:

```
cargo install sqlar
```

## Usage

### List the content of an archive

```
sqlar l path/to/file.sqlar
```

### Extract an archive

```
sqlar x path/to/file.sqlar path/to/dest/
```

### Create an archive

```
sqlar c path/to/new-archive.sqlar path/to/source/
```

## Example

The library can also be used progamatically.

### List files in an archive

```rust
use sqlar::with_each_entry;

with_each_entry("path/to/archive.sqlar", false, |entry| {
   println!("File: {}, file type: {:?}, mode: {}", entry.name, entry.filetype, entry.mode);
   Ok(())
});
```

### Create an archive

```rust
use sqlar::create;

create("path/to/new-archive.sqlar", &["path/to/source"]);
```

### Extract all files from an archive

```rust
use sqlar::extract;

extract("path/to/archive.sqlar", "path/to/dest");
```

[sqlar]: https://www.sqlite.org/sqlar.html

# License

MIT. See [LICENSE](LICENSE).
