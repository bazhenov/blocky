extern crate clap;
extern crate md5;

use std::error::Error;
use std::path::Path;
use std::{env, io};

use crate::block::Block;
use clap::{App, Arg, SubCommand};

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
        );
    let matches = app.clone().get_matches();

    match matches.subcommand() {
        ("inspect", Some(opts)) => {
            let path = opts.value_of("INPUT").unwrap();
            inspect(path)
        }
        _ => {
            app.write_help(&mut io::stdout()).unwrap();
            Ok(())
        }
    }
}

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
            hash = md5::Digest(file.file_name_hash)
        );
    }

    Ok(())
}
