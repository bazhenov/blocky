extern crate clap;
extern crate md5;
#[macro_use]
extern crate error_chain;

use crate::block::{AddFileRequest, Block};
use clap::{App, SubCommand, Values};
pub use errors::*;
use std::io::{self, Write};
use std::path::Path;

mod block;

mod errors {
    error_chain! {
        errors {
            NoFilesInBlock {

            }
            BlockCorrupted {
                description("Illegal block structure")
            }
        }
        foreign_links {
            Io(::std::io::Error);
        }
    }
}

quick_main!(application);

fn application() -> Result<()> {
    let app = App::new("block")
        .version("1.0")
        .author("Denis Bazhenov <dotsid@gmail.com>")
        .about("Block inspection utility")
        .subcommand(
            SubCommand::with_name("inspect")
                .about("Inspect block contents")
                .arg_from_usage("<INPUT>... 'Block file names to inspect'"),
        )
        .subcommand(
            SubCommand::with_name("create")
                .about("Create new block")
                .arg_from_usage("<BLOCK> 'Block file name'")
                .arg_from_usage("<INPUT>... 'file list'"),
        );

    let matches = app.clone().get_matches();
    match matches.subcommand() {
        ("inspect", Some(opts)) => {
            let path = opts.values_of("INPUT").unwrap();
            inspect(path)
        }
        ("create", Some(opts)) => {
            let paths = opts.values_of("INPUT").unwrap();
            let block = opts.value_of("BLOCK").unwrap();
            create(block, paths)
        }
        _ => {
            app.write_help(&mut io::stdout()).unwrap();
            Ok(())
        }
    }
}

/// Создает блок на основании файлов на локальной ФС
///
/// В данный момент файлы (их идентификаторы) нумеруются в блоке последовательно.
fn create(block_path: impl AsRef<Path>, files: Values) -> Result<()> {
    let files = files
        .enumerate()
        .map(|i| AddFileRequest {
            id: (i.0 + 1) as u64,
            path: i.1.as_ref(),
            // TODO разделить путь и URL
            location: i.1.as_ref(),
        })
        .collect::<Vec<_>>();
    Block::from_files(block_path, &files)
        .map(|_| ())
        .chain_err(|| "Unable to open file")
}

/// Выводит информацию о содержимом блока
fn inspect(block_paths: Values) -> Result<()> {
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    for block_path in block_paths {
        out.write_fmt(format_args!("{}\n", block_path))?;
        let block =
            Block::open(block_path).chain_err(|| format!("Fail to open block: {}", block_path))?;
        out.write_fmt(format_args!(
            "{id:>10} {size:>10} {offset:>10} {hash:>35}\n",
            id = "ID",
            size = "SIZE",
            offset = "OFFSET",
            hash = "HASH",
        ))?;

        for file in block.iter() {
            out.write_fmt(format_args!(
                "{id:>10} {size:>10} {offset:>10}    {hash:x}\n",
                id = file.id,
                size = file.size,
                offset = file.offset,
                hash = file.location_hash
            ))?;
        }
    }

    Ok(())
}
