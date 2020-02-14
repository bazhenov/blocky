use byteorder::{ReadBytesExt, WriteBytesExt, LE};
use std::convert::TryFrom;
use std::fmt::Debug;
use std::fs::File;
use std::io::Result;
use std::io::{BufReader, BufWriter, Error, ErrorKind::InvalidData, ErrorKind::NotFound, Write};
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
    pub id: u64,

    /// Размер файла в байтах
    pub size: u64,

    /// Смещение первого байта файла относительно налача файла
    pub offset: u32,

    /// MD5 контрольная сумма нормализованного абсолютого имени файла
    pub file_name_hash: Md5,
}

pub struct AddFileRequest<'a> {
    id: u64,
    path: &'a Path,
}

impl FileInfo {
    fn new(file: &AddFileRequest) -> Result<Self> {
        let mut info: Self = Default::default();
        info.id = file.id;
        info.size = file.path.metadata()?.len();

        return Ok(info);
    }
}

impl SelfSerialize for FileInfo {
    fn encode(&self, target: &mut impl WriteBytesExt) -> Result<()> {
        target.write_u64::<LE>(self.id)?;
        target.write_u64::<LE>(self.size)?;
        target.write_u32::<LE>(self.offset)?;
        target.write_all(&self.file_name_hash)?;
        Ok(())
    }

    fn decode(source: &mut impl ReadBytesExt) -> Result<Self> {
        let mut info: Self = Default::default();
        info.id = source.read_u64::<LE>()?;
        info.size = source.read_u64::<LE>()?;
        info.offset = source.read_u32::<LE>()?;
        source.read_exact(&mut info.file_name_hash)?;

        Ok(info)
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
            u32::try_from(len).map_err(|_| Error::new(InvalidData, "To much files"))?;
        target.write_u32::<LE>(file_info_len)?;

        for file_info in self.file_info.iter() {
            file_info.encode(target)?;
        }

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
    pub fn from_files(block_path: impl AsRef<Path>, files: &[AddFileRequest]) -> Result<Block> {
        let file_names = files.iter().map(|f| f.path).collect::<Vec<_>>();
        let first_missing_file = file_names.iter().find(|f| !f.is_file());
        if let Some(file) = first_missing_file {
            let message = format!("File: {} not found", file.display());
            return Err(Error::new(NotFound, message));
        }

        let file_infos = files.iter().map(FileInfo::new).collect::<Result<_>>()?;

        let mut block_file = BufWriter::new(File::create(&block_path)?);
        let block = Block {
            version: 1,
            file_info: file_infos,
        };

        block.encode(&mut block_file)?;
        block_file.flush()?;

        Ok(block)
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let f = File::open(path)?;
        let mut block_file = BufReader::new(f);

        let block = Block::decode(&mut block_file)?;
        Ok(block)
    }

    pub fn len(&self) -> usize {
        self.file_info.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &FileInfo> {
        self.file_info.iter()
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use std::fs::File;
    use std::io::{Cursor, Write};
    use tempdir;

    fn fixture(files: &[(impl AsRef<Path>, impl AsRef<[u8]>)]) -> Result<Block> {
        let tmp = tempdir::TempDir::new("rust-block-test")?;

        for (file_name, content) in files {
            let mut file = File::create(&tmp.path().join(file_name))?;
            file.write_all(content.as_ref())?;
        }

        let absolute_file_names = files
            .iter()
            .map(|i| tmp.path().join(i.0.as_ref()))
            .collect::<Vec<_>>();

        let files = absolute_file_names
            .iter()
            .enumerate()
            .map(|i| AddFileRequest {
                id: (i.0 + 1) as u64,
                path: i.1,
            })
            .collect::<Vec<_>>();

        let block_path = &tmp.path().join("test.block");
        Block::from_files(block_path, &files)?;
        Ok(Block::open(block_path)?)
    }

    #[test]
    fn should_create_empty_block() -> Result<()> {
        let block = fixture(&[("1.bin", "Hello"), ("2.bin", "World")])?;
        assert_eq!(block.len(), 2);

        let info = block.iter().collect::<Vec<_>>();
        assert!(info.iter().all(|i| i.id > 0));
        assert_eq!(info[0].size, 5);
        assert_eq!(info[1].size, 5);

        Ok(())
    }

    #[test]
    fn read_write_cycle() -> Result<()> {
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
