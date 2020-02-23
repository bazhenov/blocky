#[macro_use]
extern crate error_chain;

pub mod block;

pub mod errors {
    error_chain! {
        errors {
            NoFilesInBlock {

            }
            BlockCorrupted {
                description("Illegal block structure")
            }

            HeaderCorrupted {
                description("Header corrupted")
            }

            BlockFileAlreadyExists(path: String) {
                display("Block file already exists: {}", path)
            }
        }
        foreign_links {
            Io(::std::io::Error);
        }
    }
}
