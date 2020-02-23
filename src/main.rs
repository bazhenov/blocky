#[macro_use]
extern crate clap;
#[macro_use]
extern crate error_chain;
extern crate blocky;

use ::blocky::block::{AddFileRequest, Block};
use clap::{App, ArgMatches, SubCommand};
use std::io::{self, stdout, BufWriter, Write};

mod errors {
    error_chain! {
        foreign_links {
            Clap(::clap::Error);
            Io(::std::io::Error);
            Blocky(::blocky::errors::Error);
        }
    }
}

use errors::*;

quick_main!(application);

fn application() -> Result<()> {
    let app = App::new("block")
        .version("1.0")
        .author("Denis Bazhenov <dotsid@gmail.com>")
        .about("Block inspection utility")
        .subcommand(
            SubCommand::with_name("inspect")
                .about("Inspect block contents")
                .arg_from_usage(
                    "[verbose] -v, --verbose 'Report detailed information about each file'",
                )
                .arg_from_usage("<INPUT>... 'Block file names to inspect'"),
        )
        .subcommand(
            SubCommand::with_name("create")
                .about("Create new block")
                .arg_from_usage("<BLOCK> 'Block file name'")
                .arg_from_usage("<INPUT>... 'file list'"),
        )
        .subcommand(
            SubCommand::with_name("export")
                .about("Export file form the block")
                .arg_from_usage("<BLOCK> 'Block file name'")
                .arg_from_usage("<ID> 'File ID to be exported'"),
        );

    let matches = app.clone().get_matches();
    match matches.subcommand() {
        ("inspect", Some(opts)) => inspect(opts),
        ("create", Some(opts)) => create(opts),
        ("export", Some(opts)) => export(opts),
        _ => {
            app.write_help(&mut io::stdout()).unwrap();
            Ok(())
        }
    }
}

/// Создает блок на основании файлов на локальной ФС
///
/// В данный момент файлы (их идентификаторы) нумеруются в блоке последовательно.
fn create(opts: &ArgMatches) -> Result<()> {
    let files = opts.values_of("INPUT").unwrap();
    let block_path = opts.value_of("BLOCK").unwrap();

    let files = files
        .enumerate()
        .map(|(id, file)| AddFileRequest {
            id: (id + 1) as u64,
            path: file.as_ref(),
            // TODO разделить путь и URL
            location: file.as_ref(),
        })
        .collect::<Vec<_>>();
    Block::from_files(block_path, &files)
        .map(|_| ())
        .chain_err(|| "Unable to create block")
}

/// Выводит информацию о содержимом блока
fn inspect(opts: &ArgMatches) -> Result<()> {
    let block_paths = opts.values_of("INPUT").unwrap();
    let verbose = opts.is_present("verbose");
    let stdout = stdout();
    let mut out = BufWriter::new(stdout.lock());
    for block_path in block_paths {
        out.write_fmt(format_args!("{}\n", block_path))?;
        let block =
            Block::open(block_path).chain_err(|| format!("Fail to open block: {}", block_path))?;

        if verbose {
            out.write_fmt(format_args!(
                "{id:>9} {size:>9} {offset:>9} {location_hash:>32} {content_hash:>32} {location:}\n",
                id = "ID",
                size = "SIZE",
                offset = "OFFSET",
                location_hash = "LOCATION HASH",
                content_hash = "CONTENT HASH",
                location = "LOCATION",
            ))?;
        } else {
            out.write_fmt(format_args!(
                "{id:>9} {size:>9} {offset:>9} {location_hash:>32}\n",
                id = "ID",
                size = "SIZE",
                offset = "OFFSET",
                location_hash = "LOCATION HASH"
            ))?;
        }

        for (idx, file) in block.iter().enumerate() {
            if verbose {
                let (header, _) = block.file_at(idx).ok_or("Unable to read file from the block")?;
                out.write_fmt(format_args!(
                    "{id:>9} {size:>9} {offset:>9} {location_hash:32} {content_hash:32} {location:<}\n",
                    id = file.id,
                    size = file.size,
                    offset = file.offset,
                    location_hash = format!("{:x}", file.location_hash),
                    content_hash = format!("{:x}", header.hash),
                    location = header.location,
                ))?;
            } else {
                out.write_fmt(format_args!(
                    "{id:>9} {size:>9} {offset:>9} {location_hash:32}\n",
                    id = file.id,
                    size = file.size,
                    offset = file.offset,
                    location_hash = format!("{:x}", file.location_hash)
                ))?;
            }
        }
    }

    Ok(())
}

fn export(opts: &ArgMatches) -> Result<()> {
    let block_file = opts.value_of("BLOCK").unwrap();
    let id = value_t!(opts.value_of("ID"), u64)?;

    let block = Block::open(block_file)?;
    let (_, content) = block.file_by_id(id).ok_or(format!("File with id {} not found in a block", id))?;
    let out = stdout();
    let mut out = BufWriter::new(out.lock());
    out.write_all(&content)?;
    Ok(())
}