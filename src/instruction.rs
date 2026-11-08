use std::fmt;

use iced_x86::{MemorySize, OpKind, Register};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Instruction {
    pub inner: iced_x86::Instruction,
    pub operands: [Option<Operand>; 5],
}

impl fmt::Debug for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("{}", self.inner))
    }
}

impl Instruction {

    pub fn has_no_operands(&self) -> bool {
        for op in self.operands {
            if op.is_some() { 
                return false;
            }
        } 

        true
    }

    pub fn from_iced(ins: iced_x86::Instruction) -> Self {
        let mut operands = [None; 5];

        for (i, kind) in ins.op_kinds().enumerate() {
            assert!(i.lt(&5));

            match kind {
                OpKind::Register => {
                    let reg = match i {
                        0 => ins.op0_register(),
                        1 => ins.op1_register(),
                        2 => ins.op2_register(),
                        3 => ins.op3_register(),
                        4 => ins.op4_register(),
                        _ => unreachable!(),
                    };

                    if reg == Register::None {
                        panic!("Register kind none op");
                    }

                    operands[i] = Some(Operand::Register(reg));
                }

                // is this how it should work?
                OpKind::Immediate8 | OpKind::Immediate8_2nd => {
                    let val = ins.try_immediate(i as u32).unwrap();
                    operands[i] = Some(Operand::Immediate(ImmediateOp::U8(val as u8)));
                }

                OpKind::Immediate16 | OpKind::Immediate8to16 => {
                    let val = ins.try_immediate(i as u32).unwrap();
                    operands[i] = Some(Operand::Immediate(ImmediateOp::U16(val as u16)));
                }

                OpKind::Immediate32 | OpKind::Immediate8to32 => {
                    let val = ins.try_immediate(i as u32).unwrap();
                    operands[i] = Some(Operand::Immediate(ImmediateOp::U32(val as u32)));
                }

                OpKind::Immediate64 | OpKind::Immediate8to64 | OpKind::Immediate32to64 => {
                    let val = ins.try_immediate(i as u32).unwrap();
                    operands[i] = Some(Operand::Immediate(ImmediateOp::U64(val as u64)));
                }

                OpKind::Memory => {
                    let size = ins.memory_size();
                    let segment_prefix = ins.segment_prefix();
                    let segment = ins.memory_segment();
                    let base = ins.memory_base();
                    let displacement = match ins.memory_displ_size() {
                        8 => I32OrI64::I64(i64::from_be_bytes(
                            ins.memory_displacement64().to_be_bytes(),
                        )),
                        _ => I32OrI64::I32(i32::from_be_bytes(
                            ins.memory_displacement32().to_be_bytes(),
                        )),
                    };
                    let index = ins.memory_index();
                    let index_scale = ins.memory_index_scale();

                    operands[i] = Some(Operand::Memory(MemoryOp {
                        size,
                        segment_prefix,
                        segment,
                        base,
                        displacement,
                        index,
                        index_scale,
                    }));
                }

                _ => {
                    println!("Unknown operand kind {:?}", kind);
                    panic!("Unknown operand kind");
                }
            }
        }

        Self {
            inner: ins,
            operands,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Operand {
    Register(iced_x86::Register),
    Immediate(ImmediateOp),
    Memory(MemoryOp),
}

impl Operand {
    pub fn is_register(&self) -> bool {
        match self {
            Self::Register(_) => true,
            _ => false
        }
    }

    pub fn is_immediate(&self) -> bool {
        match self {
            Self::Immediate(_) => true,
            _ => false
        }
    }

    pub fn is_memory(&self) -> bool {
        match self {
            Self::Memory(_) => true,
            _ => false
        }
    }
}

impl fmt::Debug for Operand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Register(r) => f.write_fmt(format_args!("{:?}", r)),
            Self::Immediate(i) => f.write_fmt(format_args!("{:?}", i)),
            Self::Memory(m) => f.write_fmt(format_args!("{:?}", m)),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ImmediateOp {
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
}

impl ImmediateOp {
    pub fn to_u64(self) -> u64 {
        match self {
            Self::U8(a) => a as u64,
            Self::U16(a) => a as u64,
            Self::U32(a) => a as u64,
            Self::U64(a) => a as u64,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum I32OrI64 {
    I32(i32),
    I64(i64),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct MemoryOp {
    pub size: MemorySize,

    pub segment_prefix: iced_x86::Register,
    pub segment: iced_x86::Register,

    pub base: iced_x86::Register,
    /// displacement or the 64-bit absolute address if it’s an EIP or RIP relative memory operand
    pub displacement: I32OrI64,

    pub index: iced_x86::Register,

    // Gets the index register scale value, valid values are *1, *2, *4, *8
    pub index_scale: u32,
}
