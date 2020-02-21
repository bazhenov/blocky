use crate::errors::*;
use byteorder::{ReadBytesExt, WriteBytesExt, LE};
use md5;
use std::convert::TryFrom;
use std::fmt::Debug;
use std::fs::{File, OpenOptions};
use std::io::{
    self, BufReader, BufWriter, Error, ErrorKind::NotFound, Seek, SeekFrom, Write, Read, Cursor
};
use std::mem::size_of;
use std::ops::DerefMut;
use std::path::Path;
use memmap::{MmapOptions, Mmap};

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

#[derive(Eq, PartialEq, Debug)]
pub struct FileInfo {
    /// Глобальный идентификатор файла в системе
    pub id: u64,

    /// Размер файла в байтах
    pub size: u32,

    /// Смещение первого байта файла относительно налача файла
    pub offset: u32,

    /// MD5 контрольная сумма нормализованного абсолютого имени файла
    pub location_hash: md5::Digest,
}

pub struct AddFileRequest<'a> {
    pub id: u64,
    pub path: &'a Path,
    pub location: &'a Path,
}

impl FileInfo {
    fn new_at_offset(file: &AddFileRequest, offset: u32) -> Result<Self> {
        Ok(Self {
            id: file.id,
            size: file.path.metadata()?.len() as u32,
            offset,
            location_hash: md5::compute(file.location.to_str().unwrap()),
        })
    }
}

impl SelfSerialize for FileInfo {
    fn encode(&self, target: &mut impl WriteBytesExt) -> Result<()> {
        target.write_u64::<LE>(self.id)?;
        target.write_u32::<LE>(self.size)?;
        target.write_u32::<LE>(self.offset)?;
        target.write_all(self.location_hash.as_ref())?;
        Ok(())
    }

    fn decode(source: &mut impl ReadBytesExt) -> Result<Self> {
        let id = source.read_u64::<LE>()?;
        let size = source.read_u32::<LE>()?;
        let offset = source.read_u32::<LE>()?;
        let mut location_hash = md5::Digest([0; 16]);
        source.read_exact(location_hash.deref_mut())?;

        Ok(Self {
            id,
            size,
            offset,
            location_hash,
        })
    }
}

/// Группа файлов сохраненных на диске локально.
///
/// По своей сути блок напоминает tar-архив. За тем лишь исключением,
/// что ключевая информация необходимая для поиска файла (см. [`FileInfo`]) размещена компактно в
/// начале файла. Это позволяет эффективно хранить эту информацию в памяти.
///
/// ## Анатомия блока
/// ### Заголовок
/// ```text
/// +-------+-------+-------+-------+-------+-------+
/// |                     BYTES                     |
/// +-------+-------+-------+-------+-------+-------+
/// |   1   |   2   |   3   |   4   |   5   |   6   |
/// +-------+-------+-------+-------+-------+-------+
/// +    version    |              size             |
/// +-------+-------+-------+-------+-------+-------+
/// ```
/// * `version` – информация о версии формата блока (2 байта);
/// * `size` – количество файлов в блоке
///
/// ### Блок метаинформации
/// В блоке метаинформации записано по одной структуре [`FileInfo`] для каждого файла в блоке.
///
/// Каждая запись сохранена следующим образом:
/// ```text
/// +-------+-------+-------+-------+-------+-------+-------+-------+
/// |                             BYTES                             |
/// +-------+-------+-------+-------+-------+-------+-------+-------+
/// |   1   |   2   |   3   |   4   |   5   |   6   |   7   |   8   |
/// +-------+-------+-------+-------+-------+-------+-------+-------+
/// +                              id                               |
/// +-------+-------+-------+-------+-------+-------+-------+-------+
/// +                             size                              |
/// +-------+-------+-------+-------+-------+-------+-------+-------+
/// +            offset             |             hash              |
/// +-------+-------+-------+-------+-------+-------+-------+-------+
/// +                              hash                             |
/// +-------+-------+-------+-------+-------+-------+-------+-------+
/// +              hash             |
/// +-------+-------+-------+-------+
/// ```
/// * `id` – глобальный идентификатор файла в системе;
/// * `size` – размер файла в байтах;
/// * `offset` – смещение первого байта файла относительно начала блока. Таким образом,
/// смещение всегда больше чем длина заголовков блока.
/// * `hash` – MD5-хеш URL-файла (например, `/path/to/image.jpeg`).
///
/// [`FileInfo`]: struct.FileInfo.html
#[derive(Debug, Default, Eq, PartialEq)]
pub struct BlockHeader {
    version: u16,
    file_info: Vec<FileInfo>,
}

pub struct Block {
    header: BlockHeader,
    mmap: Mmap
}

impl SelfSerialize for BlockHeader {
    fn encode(&self, target: &mut impl WriteBytesExt) -> Result<()> {
        target.write_u16::<LE>(self.version)?;
        let len = self.file_info.len();
        let file_info_len = u32::try_from(len).chain_err(|| "File id can't fit in u32")?;
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

const BLOCK_PAGE_SIZE: u32 = 1024;

impl Block {
    pub fn from_files(block_path: impl AsRef<Path>, files: &[AddFileRequest]) -> Result<Block> {
        if files.is_empty() {
            bail!(ErrorKind::NoFilesInBlock);
        }
        let file_names = files.iter().map(|f| f.path).collect::<Vec<_>>();
        let first_missing_file = file_names.iter().find(|f| !f.is_file());
        if let Some(file) = first_missing_file {
            let message = format!("File: {} not found", file.display());
            return Err(Error::new(NotFound, message).into());
        }

        // Расчитываем смещения файлов в блоке и формируем заголовок
        let header_size = (size_of::<Block>() + files.len() * size_of::<FileInfo>()) as u32;
        let mut next_file_offset = round_up_to(header_size, BLOCK_PAGE_SIZE);
        let mut file_infos = vec![];
        for file in files {
            let file_info = FileInfo::new_at_offset(file, next_file_offset)?;
            next_file_offset = round_up_to(next_file_offset + file_info.size, BLOCK_PAGE_SIZE);
            file_infos.push(file_info);
        }

        let header = BlockHeader {
            version: 1,
            file_info: file_infos,
        };

        // Записываем блок заголовков в память, чтобы замерять суммарный размер заголовка

        let mut block_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&block_path)?;
        header
            .encode(&mut block_file)
            .chain_err(|| "Unable to write block header")?;

        for (file, req) in header.file_info.iter().zip(files) {
            block_file.set_len(file.offset.into())?;
            block_file.seek(SeekFrom::End(0))?;
            let mut writer = BufWriter::new(&block_file);
            let mut reader = BufReader::new(File::open(req.path)?);
            io::copy(&mut reader, &mut writer)
                .chain_err(|| "Unable to copy a file to the block")?;
        }

        // Записываем заголовки в блок
        block_file.seek(SeekFrom::Start(0))?;

        block_file.flush()?;

        Self::open(block_path)
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let f = File::open(&path)?;
        let mut block_file = BufReader::new(&f);

        let header = BlockHeader::decode(&mut block_file).chain_err(|| ErrorKind::BlockCorrupted)?;
        let mmap = unsafe { MmapOptions::new().map(&f)? };
        Ok(Block { header, mmap })
    }

    pub fn file_at(&self, idx: usize) -> Result<&impl AsRef<[u8]>> {
        let info = &self.header.file_info[idx];
        let start = info.offset as usize;
        let end = (info.offset + info.size) as usize;
        Ok(&self.mmap.as_ref()[start..end])
    }

    pub fn len(&self) -> usize {
        self.header.file_info.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &FileInfo> {
        self.header.file_info.iter()
    }
}

/// Округляет целое беззнаковое целое `value` до следующего кратного `base`.
///
/// Например:
/// ```rust
/// use blocky::block::round_up_to;
/// assert_eq!(round_up_to(10, 12), 12);
/// assert_eq!(round_up_to(12, 12), 12);
/// assert_eq!(round_up_to(13, 12), 24);
///
/// assert_eq!(round_up_to(1, 2048), 2048);
/// assert_eq!(round_up_to(2048, 2048), 2048);
/// assert_eq!(round_up_to(2049, 2048), 2 * 2048);
/// assert_eq!(round_up_to(2 * 2048 + 1, 2048), 3 * 2048);
/// ```
pub fn round_up_to(value: u32, base: u32) -> u32 {
    let reminder = value % base;
    if reminder == 0 {
        value
    } else {
        value + base - reminder
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
        let tmp = tmp.path();

        let mut absolute_file_names = vec![];
        let mut locations = vec![];
        let root = Path::new("/");

        for (file_name, content) in files {
            let path = tmp.join(file_name);
            let mut file = File::create(&path)?;
            file.write_all(content.as_ref())?;

            locations.push(root.join(file_name));
            absolute_file_names.push(path);
        }

        let add_requests = absolute_file_names
            .iter()
            .zip(locations.iter())
            .enumerate()
            .map(|(id, (path, location))| AddFileRequest {
                id: (id + 1) as u64,
                path,
                location,
            })
            .collect::<Vec<_>>();

        let block_path = tmp.join("test.block");
        Block::from_files(&block_path, &add_requests)?;
        Ok(Block::open(&block_path)?)
    }

    #[test]
    #[should_panic]
    fn block_could_not_be_created_twice() {
        let block_path_name = "./target/test.block";
        Block::from_files(
            block_path_name,
            &[AddFileRequest {
                id: 1,
                path: Path::new("./foo"),
                location: Path::new("./foo"),
            }],
        )
        .unwrap();

        Block::from_files(
            block_path_name,
            &[AddFileRequest {
                id: 1,
                path: Path::new("./foo"),
                location: Path::new("./foo"),
            }],
        )
        .unwrap();
    }

    #[test]
    fn should_be_able_to_create_block_and_return_file_info() -> Result<()> {
        let block = fixture(&[("1.bin", "Hello"), ("2.bin", "World")])?;
        assert_eq!(block.len(), 2);

        let info = block.iter().collect::<Vec<_>>();
        assert!(info.iter().all(|i| i.id > 0));

        assert_eq!(info[0].size, 5);
        assert_eq!(
            format!("{:x}", info[0].location_hash),
            "d0e14e5f5e76ec1a00e5fb02e4b47d9a"
        );

        assert_eq!(info[1].size, 5);
        assert_eq!(
            format!("{:x}", info[1].location_hash),
            "475e9b6e16f464efea93b8312b90ec02"
        );

        Ok(())
    }

    #[test]
    fn should_be_able_to_return_block_content() -> Result<()> {
        let block = fixture(&[("one.txt", "text-content")])?;
        let reader = block.file_at(0)?;

        assert_eq!("901a84918e4d5121ceae18305d2cd938", format!("{:x}", md5::compute(reader)));
        
        Ok(())
    }

    #[test]
    fn should_fail_if_no_file_are_given() -> Result<()> {
        let block = Block::from_files("./test.bin", &[]);
        assert!(block.is_err());
        Ok(())
    }

    #[test]
    fn read_write_cycle() -> Result<()> {
        test_read_write_cycle(&BlockHeader {
            version: 3,
            file_info: vec![FileInfo {
                id: 1,
                size: 15,
                offset: 0,
                location_hash: md5::Digest([0u8; 16]),
            }],
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
