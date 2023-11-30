use crate::CPUType;
use crate::RcReader;
use crate::Result;
use crate::cpu_constants::*;
use crate::reader::MutReaderRef;

use scroll::{IOread};

use std::fmt::Debug;
use std::io::{Seek, SeekFrom};
use std::mem::size_of;

use crate::auto_enum_fields::*;
use schnauzer_derive::AutoEnumFields;

// LC_THREAD_FLAVOR_HEADER_SIZE = sizeof(thread_command.flavor) + sizeof(thread_command.count)
const LC_THREAD_FLAVOR_HEADER_SIZE: u32 = size_of::<u32>() as u32 + size_of::<u32>() as u32;

/// `thread_command`
#[repr(C)]
#[derive(AutoEnumFields,Debug)]
pub struct LcThread {
    reader: RcReader,

    cmdsize: u32,
    base_offset: usize,
    endian: scroll::Endian,
    cpu_type: CPUType,
}

impl LcThread {
    pub(super) fn parse(reader: RcReader, cmdsize: u32, base_offset: usize, endian: scroll::Endian, cpu_type: CPUType) -> Result<Self> {
        Ok(LcThread { reader, cmdsize, base_offset, endian, cpu_type })
    }

    pub fn flavor_iterator(&self) -> FlavorIterator {
        FlavorIterator::new(self.reader.clone(), self.cmdsize, self.base_offset, self.endian, self.cpu_type)
    }
}

#[repr(C)]
pub struct LcThreadFlavor {
    pub flavor: u32,
    pub count: u32,
    pub state: FlavorState,
}

impl LcThreadFlavor {
    pub(super) fn parse(reader: &RcReader, base_offset: usize, endian: scroll::Endian, cpu_type: CPUType) -> Result<Option<Self>> {
        let mut reader_mut = reader.borrow_mut();
        reader_mut.seek(SeekFrom::Start(base_offset as u64))?;

        let flavor: u32 = reader_mut.ioread_with(endian)?;
        let count: u32 = reader_mut.ioread_with(endian)?;
        let state = FlavorState::parse(&mut reader_mut, endian, flavor, cpu_type)?;

        if flavor == 0 && count == 0 {
            // We reached the end of the list
            return Ok(None);
        }

        Ok(Some(LcThreadFlavor { flavor, count, state }))
    }

    fn calculate_flavor_size(&self) -> u32 {
        // the size of a flavor is based on the following:
        // flavor_size = LC_THREAD_FLAVOR_HEADER_SIZE + sizeof(thread_command.state)

        // count * sizeof(uint32_t) is equalivent to sizeof(thread_command.state)
        LC_THREAD_FLAVOR_HEADER_SIZE + self.count * size_of::<u32>() as u32
    }
}

impl AutoEnumFields for LcThreadFlavor {
    fn all_fields(&self) -> Vec<Field> {
        let mut fields: Vec<Field> =  Vec::new();
        fields.push(Field { name: "flavor".to_string(), value: self.flavor.to_string() });
        fields.push(Field { name: "count".to_string(), value: self.count.to_string() });
        // We manually print out the state with the `handle_thread_state` method

        fields
    }
}

impl Debug for LcThreadFlavor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LcThreadFlavor")
            .field("flavor", &self.flavor)
            .field("count", &self.count)
            .finish()
    }
}

pub struct FlavorIterator {
    reader: RcReader,
    base_offset: usize,
    cmdsize: u32,
    endian: scroll::Endian,
    cpu_type: CPUType,

    current: u32,
}

impl FlavorIterator {
    fn new(reader: RcReader, cmdsize: u32, base_offset: usize, endian: scroll::Endian, cpu_type: CPUType) -> Self {        
        FlavorIterator {
            reader,
            base_offset,
            cmdsize,
            endian,
            cpu_type,
            current: 0,
        }
    }
}

impl Iterator for FlavorIterator {
    type Item = LcThreadFlavor;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current >= self.cmdsize {
            return None;
        }

        let offset = self.base_offset + self.current as usize;

        match LcThreadFlavor::parse(&self.reader, offset as usize, self.endian, self.cpu_type) {
            Ok(Some(lc_thread_flavor)) => {
                self.current += lc_thread_flavor.calculate_flavor_size();
                Some(lc_thread_flavor)
            },

            Ok(None) => {
                self.current = self.cmdsize;
                None
            },

            Err(_) => None,
        }
    }
}

#[derive(Debug)]
pub enum FlavorState {
    Unknown
}

impl FlavorState {
    fn parse(reader_mut: &mut MutReaderRef, endian: scroll::Endian, flavor: u32, cpu_type: CPUType) -> Result<FlavorState> {
        match cpu_type {
            CPU_TYPE_ARM64 => {
                match flavor {
                    _ => Ok(FlavorState::Unknown)
                }
            },
            
            _ => Ok(FlavorState::Unknown)
        }
    }

    pub fn all_fields_with_header(&self) -> Option<(&str,Vec<Field>)> {
        None
    }
}

