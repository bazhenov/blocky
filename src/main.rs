use std::env;
use std::error::Error;

mod block;

fn main() -> Result<(), Box<dyn Error>> {
    for arg in env::args() {
        println!("{}", arg);
    }
    Ok(())
}
