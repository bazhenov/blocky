use byteorder::{ReadBytesExt, WriteBytesExt, LE};
use std::convert::TryFrom;
use std::fmt::Debug;
use std::io::Result;
use std::io::{Error, ErrorKind::InvalidData, ErrorKind::NotFound};
use std::path::Path;

type Md5 = [u8; 16];

/// Трейт позволяющий произвольному типу самостоятельно реализовать логику
/// собственной сераилизации/десериализации используя библиотеку byteorder.
///
/// Используется два метода: encode/decode для сериализации и десериализации
/// соответственно
trait SelfSerialize {
    fn encode(&self, target: &mut impl WriteBytesExt) -> Result<()>;
    fn decode(source: &mut impl ReadBytesExt) -> Result<Self>
    where
        Self: Sized;
}

#[derive(Eq, PartialEq, Debug, Default)]
pub struct FileInfo {
    /// Глобальный идентификатор файла в системе
    id: u64,

    /// Размер файла в байтах
    size: u32,

    /// Смещение первого байта файла относительно налача файла
    offset: u32,

    /// MD5 контрольная сумма нормализованного абсолютого имени файла
    file_name_hash: Md5,
}

impl FileInfo {
    fn from_file(path: impl AsRef<Path>) -> Self {
        Default::default()
    }
}

impl SelfSerialize for FileInfo {
    fn decode(source: &mut impl ReadBytesExt) -> Result<Self> {
        let mut info: Self = Default::default();
        info.id = source.read_u64::<LE>()?;
        info.size = source.read_u32::<LE>()?;
        info.offset = source.read_u32::<LE>()?;
        source.read_exact(&mut info.file_name_hash)?;

        Ok(info)
    }

    fn encode(&self, target: &mut impl WriteBytesExt) -> Result<()> {
        target.write_u64::<LE>(self.id)?;
        target.write_u32::<LE>(self.size)?;
        target.write_u32::<LE>(self.offset)?;
        target.write_all(&self.file_name_hash)?;
        Ok(())
    }
}

#[derive(Debug, Default, Eq, PartialEq)]
pub struct Block {
    version: u16,
    file_info: Vec<FileInfo>,
}

impl SelfSerialize for Block {
    fn encode(&self, target: &mut impl WriteBytesExt) -> Result<()> {
        target.write_u16::<LE>(self.version)?;
        let len = self.file_info.len();
        let file_info_len =
            u32::try_from(len).map_err(|_| Error::new(InvalidData, "To mush files"))?;
        target.write_u32::<LE>(file_info_len)?;

        Ok(())
    }

    fn decode(source: &mut impl ReadBytesExt) -> Result<Self> {
        let mut header: Self = Default::default();
        header.version = source.read_u16::<LE>()?;
        let file_info_len = source.read_u32::<LE>()?;
        header.file_info = vec![];
        for _ in 0..file_info_len {
            let file_info = FileInfo::decode(source)?;
            header.file_info.push(file_info);
        }

        Ok(header)
    }
}

impl Block {
    pub fn from_files<T: AsRef<Path>, K: AsRef<Path>>(work_dir: &K, files: &[T]) -> Result<Block> {
        let absolute_file_names = files
            .iter()
            .map(|f| work_dir.as_ref().join(f))
            .collect::<Vec<_>>();
        let first_missing_file = absolute_file_names.iter().find(|f| !f.is_file());
        if let Some(file) = first_missing_file {
            let message = format!("File: {} not found", file.display());
            return Err(Error::new(NotFound, message));
        }

        let file_infos = absolute_file_names
            .iter()
            .map(FileInfo::from_file)
            .collect();
        Ok(Block {
            version: 1,
            file_info: file_infos,
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
    use std::io::{Cursor, Write};
    use tempdir;

    fn fixture(files: &[(impl AsRef<Path>, &str)]) -> Result<Block> {
        let tmp = tempdir::TempDir::new("rust-block-test")?;
        let mut f = vec![];

        for (file_name, content) in files {
            write!(File::create(&tmp.path().join(file_name))?, "{}", content)?;
            f.push(file_name.as_ref());
        }

        let file_names = files.iter().map(|i| &i.0).collect::<Vec<_>>();

        Ok(Block::from_files(&tmp.path(), &file_names)?)
    }

    #[test]
    fn should_create_empty_block() -> Result<()> {
        let block = fixture(&[("1.bin", "Hello"), ("2.bin", "World")])?;
        assert_eq!(block.len(), 2);

        Ok(())
    }

    #[test]
    fn foo() -> Result<()> {
        test_read_write_cycle(&Block {
            version: 3,
            file_info: vec![Default::default()],
        })
    }

    fn test_read_write_cycle<T>(target: &T) -> Result<()>
    where
        T: SelfSerialize + Eq + Debug,
    {
        let mut cursor = Cursor::new(vec![0; 1024]);

        target.encode(&mut cursor)?;
        cursor.set_position(0);

        let target_copy = T::decode(&mut cursor)?;
        assert_eq!(target, &target_copy);

        Ok(())
    }
}
