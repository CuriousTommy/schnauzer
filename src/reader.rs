use std::{path::Path, io::{self, Read, Seek, SeekFrom, BufReader, BufRead, Cursor, Write}, fs::File, fmt::Debug};
use crate::fmt_ext;

use super::result::*;

use std::rc::Rc;
use std::cell::RefCell;
use std::cell::RefMut;
pub(super) type RcReader = super::RcCell<Reader>;
pub(super) type MutReaderRef<'a> = RefMut<'a, Reader>;

#[derive(Debug)]
pub struct Reader {
    read: ReaderType,
}

pub enum ReaderBuildOption<'a> {
    File(&'a Path),
    Memory(&'a [u8])
}

#[derive(Debug)]
enum ReaderType {
    BufferFile(BufReader<File>),
    CursorMemory(Cursor<Vec<u8>>)
}

impl Reader {
    pub(super) fn build(option: ReaderBuildOption) -> Result<RcReader> {
        match option {
            ReaderBuildOption::File(path) => {
                let file = File::open(path)?;
                let read = ReaderType::BufferFile(BufReader::new(file));
                Ok(Rc::new(RefCell::new(Reader { read })))
            }

            ReaderBuildOption::Memory(memory) => {
                let mut memory_cpy = Vec::new();
                memory_cpy.write_all(memory).unwrap();
                let read = ReaderType::CursorMemory(Cursor::new(memory_cpy));
                Ok(Rc::new(RefCell::new(Reader { read })))
            }
        }
    }
}

impl Seek for Reader {
    fn seek(&mut self, style: SeekFrom) -> io::Result<u64> {
        match &mut self.read {
            ReaderType::BufferFile(read) => read.seek(style),
            ReaderType::CursorMemory(read) => read.seek(style),
        }
    }
}

impl Read for Reader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match &mut self.read {
            ReaderType::BufferFile(read) => read.read(buf),
            ReaderType::CursorMemory(read) => read.read(buf),
        }
    }
}

impl Reader {
    pub fn read_zero_terminated_string(&mut self) -> Result<String> {
        let mut buf = Vec::new();

        match &mut self.read {
            ReaderType::BufferFile(read) => read.read_until(0, &mut buf)?,
            ReaderType::CursorMemory(read) => read.read_until(0, &mut buf)?,
        };

        Ok(fmt_ext::printable_string(&buf))
    }
}