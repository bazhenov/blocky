use memmap::Mmap;
use std::path::Path;

type Md5 = [u8; 16];

#[derive(Copy, Clone)]
pub struct FileInfo {
}

pub struct Block {
  file_info: Vec<FileInfo>,
  mmap: Option<Mmap>,
}

impl Block {

  pub fn from_files<T: AsRef<Path>>(files: &[T]) -> Block {
    Block {
      file_info: vec![FileInfo{}; files.len()],
      mmap: None
    }
  }

  pub fn len(&self) -> usize {
    self.file_info.len()
  }
}

#[cfg(test)]
mod tests {

  use super::*;
  use tempdir;
  use std::io::{self, Write};
  use std::fs::File;

  #[test]
  fn should_create_empty_block() -> Result<(), io::Error> {
    let tmp = tempdir::TempDir::new("rust-block-test")?;
    let mut files = vec![];
    
    let p = tmp.path().join("1.bin");
    writeln!(File::create(&p)?, "Hello")?;
    files.push(p);

    let p = tmp.path().join("2.bin");
    writeln!(File::create(&p)?, "World")?;
    files.push(p);

    let block = Block::from_files(&files[..]);
    assert_eq!(block.len(), 2);

    Ok(())
  }
}