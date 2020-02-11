use memmap::MmapOptions;
use std::fs::File;
use std::error::Error;
use std::env;

mod block;

fn main() -> Result<(), Box<dyn Error>> {
    for arg in env::args() {
        println!("{}", arg);
    }
    Ok(())
}
