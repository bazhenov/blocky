extern crate clap;
extern crate md5;

use std::error::Error;
use std::io;
use std::path::Path;

use clap::{App, SubCommand, Values};

use crate::block::{AddFileRequest, Block};

mod block;

fn main() -> Result<(), Box<dyn Error>> {
    let app = App::new("block")
        .version("1.0")
        .author("Denis Bazhenov <dotsid@gmail.com>")
        .about("Block inspection utility")
        .subcommand(
            SubCommand::with_name("inspect")
                .about("Inspect block contents")
                .arg_from_usage("<INPUT> 'Block file name to inspect'"),
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
            let path = opts.value_of("INPUT").unwrap();
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
fn create(block_path: impl AsRef<Path>, files: Values) -> Result<(), Box<dyn Error>> {
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
        .map_err(Into::into)
}

/// Выводит информацию о содержимом блока
fn inspect(block_path: impl AsRef<Path>) -> Result<(), Box<dyn Error>> {
    let block = Block::open(block_path)?;
    println!(
        "{id:>10} {size:>10} {offset:>10} {hash:>35}",
        id = "ID",
        size = "SIZE",
        offset = "OFFSET",
        hash = "HASH",
    );

    for file in block.iter() {
        println!(
            "{id:>10} {size:>10} {offset:>10}    {hash:x}",
            id = file.id,
            size = file.size,
            offset = file.offset,
            hash = file.location_hash
        );
    }

    Ok(())
}
