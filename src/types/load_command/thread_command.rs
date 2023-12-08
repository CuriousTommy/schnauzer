use crate::CPUType;
use crate::{Hu32,Hu64};
use crate::RcReader;
use crate::Result;
use crate::cpu_constants::*;
use crate::reader::MutReaderRef;

use scroll::{IOread,SizeWith};

use std::fmt::Debug;
use std::io::{Seek, SeekFrom};
use std::mem::size_of;

use crate::auto_enum_fields::*;
use schnauzer_derive::AutoEnumFields;

// LC_THREAD_FLAVOR_HEADER_SIZE = sizeof(thread_command.flavor) + sizeof(thread_command.count)
const LC_THREAD_FLAVOR_HEADER_SIZE: u32 = size_of::<u32>() as u32 + size_of::<u32>() as u32;

pub const X86_THREAD_STATE32: u32 = 1;
pub const X86_THREAD_STATE64: u32 = 4;

pub const ARM_THREAD_STATE64: u32 = 6;
pub const ARM_EXCEPTION_STATE64: u32 = 7;

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
    X86ThreadState32(X86ThreadState32),
    X86ThreadState64(X86ThreadState64),
    ArmThreadState64(ArmThreadState64),
    ArmExceptionState64(ArmExceptionState64),
    Unknown
}

impl FlavorState {
    fn parse(reader_mut: &mut MutReaderRef, endian: scroll::Endian, flavor: u32, cpu_type: CPUType) -> Result<FlavorState> {
        match cpu_type {
            CPU_TYPE_I386 => {
                match flavor {
                    X86_THREAD_STATE32 => {
                        let state = reader_mut.ioread_with(endian)?;
                        Ok(FlavorState::X86ThreadState32(state))
                    }
                    _ => Ok(FlavorState::Unknown)
                }
            },

            CPU_TYPE_X86_64 => {
                match flavor {
                    X86_THREAD_STATE64 => {
                        let state = reader_mut.ioread_with(endian)?;
                        Ok(FlavorState::X86ThreadState64(state))
                    }
                    _ => Ok(FlavorState::Unknown)
                }
            },

            CPU_TYPE_ARM64 => {
                match flavor {
                    ARM_THREAD_STATE64 => {
                        let state = ArmThreadState64::parse(reader_mut, endian)?;
                        Ok(FlavorState::ArmThreadState64(state))
                    },
                    ARM_EXCEPTION_STATE64 => {
                        let state = reader_mut.ioread_with(endian)?;
                        Ok(FlavorState::ArmExceptionState64(state))
                    }
                    _ => Ok(FlavorState::Unknown)
                }
            },

            _ => Ok(FlavorState::Unknown)
        }
    }

    pub fn all_fields_with_header(&self) -> Option<(&str,Vec<Field>)> {
        let name;
        let fields;

        match self {
            FlavorState::X86ThreadState32(state) => {
                name = "STRUCT_X86_THREAD_STATE32";
                fields = state.all_fields();
            },
            FlavorState::X86ThreadState64(state) => {
                name = "STRUCT_X86_THREAD_STATE64";
                fields = state.all_fields();
            }
            FlavorState::ArmThreadState64(state) => {
                name = "STRUCT_ARM_THREAD_STATE64";
                fields = state.all_fields();
            },
            FlavorState::ArmExceptionState64(state) => {
                name = "STRUCT_ARM_EXCEPTION_STATE64";
                fields = state.all_fields();
            }
            FlavorState::Unknown => return None,
        }

        Some((name,fields))
    }
}

#[derive(Debug,IOread,SizeWith,AutoEnumFields)]
pub struct X86ThreadState32
{
    pub eax: Hu32,
    pub ebx: Hu32,
    pub ecx: Hu32,
    pub edx: Hu32,
    pub edi: Hu32,
    pub esi: Hu32,
    pub ebp: Hu32,
    pub esp: Hu32,
    pub ss: Hu32,
    pub eflags: Hu32,
    pub eip: Hu32,
    pub cs: Hu32,
    pub ds: Hu32,
    pub es: Hu32,
    pub fs: Hu32,
    pub gs: Hu32,
}

#[derive(Debug,IOread,SizeWith,AutoEnumFields)]
pub struct X86ThreadState64 {
	pub rax: Hu64,
	pub rbx: Hu64,
	pub rcx: Hu64,
	pub rdx: Hu64,
	pub rdi: Hu64,
	pub rsi: Hu64,
	pub rbp: Hu64,
	pub rsp: Hu64,
	pub r8: Hu64,
	pub r9: Hu64,
	pub r10: Hu64,
	pub r11: Hu64,
	pub r12: Hu64,
	pub r13: Hu64,
	pub r14: Hu64,
	pub r15: Hu64,
	pub rip: Hu64,
	pub rflags: Hu64,
	pub cs: Hu64,
	pub fs: Hu64,
    pub gs: Hu64,
}

#[derive(Debug)]
pub struct ArmThreadState64 {
    pub x: [Hu64; 29],
    pub fp: Hu64,
    pub lr: Hu64,
    pub sp: Hu64,
    pub pc: Hu64,
    pub cpsr: Hu32,
    pub flags: Hu32,
}

impl ArmThreadState64 {
    // Workaround due to the size of ArmThreadState64 being larger then the 256 buffer limit...
    fn parse(reader_mut: &mut MutReaderRef, endian: scroll::Endian) -> Result<ArmThreadState64>{
        let mut x: [Hu64; 29] = [Hu64(0); 29];
        for i in 0..29 {
            x[i] = reader_mut.ioread_with(endian)?;
        }

        let fp: Hu64 = reader_mut.ioread_with(endian)?;
        let lr: Hu64 = reader_mut.ioread_with(endian)?;
        let sp: Hu64 = reader_mut.ioread_with(endian)?;
        let pc: Hu64 = reader_mut.ioread_with(endian)?;
        let cpsr: Hu32 = reader_mut.ioread_with(endian)?;
        let flags: Hu32 = reader_mut.ioread_with(endian)?;

        Ok(ArmThreadState64 { x, fp, lr, sp, pc, cpsr, flags })
    }
}

impl AutoEnumFields for ArmThreadState64 {
    fn all_fields(&self) -> Vec<Field> {
        let mut fields: Vec<Field> = Vec::new();

        for i in 0..29 {
            fields.push(Field { name: format!("x{}", i), value: self.x[i].to_string() });
        }

        fields.push(Field { name: "fp".to_string(), value: self.fp.to_string() });
        fields.push(Field { name: "lr".to_string(), value: self.lr.to_string() });
        fields.push(Field { name: "sp".to_string(), value: self.sp.to_string() });
        fields.push(Field { name: "pc".to_string(), value: self.pc.to_string() });
        fields.push(Field { name: "cpsr".to_string(), value: self.cpsr.to_string() });
        fields.push(Field { name: "flags".to_string(), value: self.flags.to_string() });

        fields
    }
}

#[derive(Debug,IOread,SizeWith,AutoEnumFields)]
pub struct ArmExceptionState64 {
	pub far: Hu64,
	pub esr: Hu32,
	pub exception: Hu32,
}