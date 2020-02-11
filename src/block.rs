use std::borrow::Borrow;
use std::io::{Error, ErrorKind::NotFound};
use std::path::Path;

#[derive(Copy, Clone)]
pub struct FileInfo {}

pub struct Block {
    file_info: Vec<FileInfo>,
}

impl Block {
    pub fn from_files<T: AsRef<Path>>(work_dir: &T, files: &[T]) -> Result<Block, Error> {
        let absolute_file_names = files
            .iter()
            .map(|f| work_dir.as_ref().join(f))
            .collect::<Vec<_>>();
        let first_missing_file = absolute_file_names.iter().find(|f| !f.is_file());
        if let Some(file) = first_missing_file {
            let message = format!("File: {} not found", file.display());
            return Err(Error::new(NotFound, message));
        }

        Ok(Block {
            file_info: vec![FileInfo {}; files.len()],
        })
    }

    pub fn len(&self) -> usize {
        self.file_info.len()
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use std::fs::File;
    use std::io::{self, Write};
    use tempdir;

    #[test]
    fn should_create_empty_block() -> Result<(), io::Error> {
        let tmp = tempdir::TempDir::new("rust-block-test")?;
        let mut files = vec![];

        let p = Path::new("1.bin");
        writeln!(File::create(&tmp.path().join(p))?, "Hello")?;
        files.push(p);

        let p = Path::new("2.bin");
        writeln!(File::create(&tmp.path().join(p))?, "World")?;
        files.push(p);

        let block = Block::from_files(&tmp.path(), &files[..])?;
        assert_eq!(block.len(), 2);

        Ok(())
    }
}
