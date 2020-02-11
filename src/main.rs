use memmap::MmapOptions;
use std::env;
use std::error::Error;
use std::fs::File;

mod block;

fn main() -> Result<(), Box<dyn Error>> {
    for arg in env::args() {
        println!("{}", arg);
    }
    Ok(())
}
