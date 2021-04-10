use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::Result;
use argh::FromArgs;
use chrono::NaiveDateTime;
use log::LevelFilter;
use sqlar::{with_each_file, FileType};
use tabwriter::TabWriter;

#[derive(FromArgs, PartialEq, Debug)]
/// sqlar utility
struct Command {
    #[argh(subcommand)]
    nested: Subcommand,

    /// verbose output
    #[argh(switch, short = 'V')]
    verbose: bool,
}

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand)]
enum Subcommand {
    Extract(Extract),
    Create(Create),
    List(List),
}

/// Extract files from archive
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "x")]
struct Extract {
    /// archive file
    #[argh(positional)]
    archive: PathBuf,

    /// destination to extract to (optional).
    /// Defaults to the archive file name without extension.
    #[argh(positional)]
    destination: Option<PathBuf>,
}

/// Create a new archive
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "c")]
struct Create {
    /// archive file
    #[argh(positional)]
    archive: PathBuf,

    /// file or directory to include
    #[argh(positional)]
    path: PathBuf,

    /// additional files to include
    #[argh(positional)]
    paths: Vec<PathBuf>,
}

/// List contents of archive
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "l")]
struct List {
    /// archive file
    #[argh(positional)]
    archive: PathBuf,
}

fn main() {
    match real_main() {
        Ok(()) => {}
        Err(e) => {
            eprintln!("An error occured:");
            eprintln!("{}", e);
        }
    }
}

fn real_main() -> Result<()> {
    let cmd: Command = argh::from_env();

    env_logger::Builder::new()
        .filter_level(if cmd.verbose {
            LevelFilter::Info
        } else {
            LevelFilter::Warn
        })
        .init();

    match cmd.nested {
        Subcommand::Extract(x) => {
            let archive = &x.archive;
            let destination = x
                .destination
                .or_else(|| archive.file_stem().map(PathBuf::from));
            let destination = match destination {
                Some(d) => d,
                None => anyhow::bail!("missing destination"),
            };
            log::info!(
                "Extracting {} to {}/",
                archive.display(),
                destination.display()
            );
            sqlar::extract(&archive, &destination)?
        }
        Subcommand::Create(c) => {
            let mut paths = vec![c.path];
            paths.extend_from_slice(&c.paths);
            log::info!(
                "Creating new archive {} with files: {:?}",
                c.archive.display(),
                paths
            );
            sqlar::create(&c.archive, &paths)?
        }
        Subcommand::List(l) => list(&*l.archive)?,
    }

    Ok(())
}

/// List all files in the SQL archive
pub fn list(path: &Path) -> Result<()> {
    let stdout = io::stdout();
    let handle = stdout.lock();
    let mut tw = TabWriter::new(handle);
    writeln!(&mut tw, "Name\tType\tMode\tModified\tSize (Compressed)").unwrap();
    writeln!(&mut tw, "====\t====\t====\t========\t=================").unwrap();

    with_each_file(path, false, |entry| {
        let ts = NaiveDateTime::from_timestamp(entry.mtime, 0);
        writeln!(
            &mut tw,
            "{}\t{:?}\t{:o}\t{} UTC\t{} ({:.1}%)",
            entry.name,
            entry.filetype,
            entry.mode,
            ts,
            entry.size,
            if entry.filetype == FileType::File {
                (entry.compressed_size as f64 / entry.size as f64) * 100.0
            } else {
                0.0
            },
        )
        .unwrap();
        Ok(())
    })?;

    tw.flush().unwrap();

    Ok(())
}
