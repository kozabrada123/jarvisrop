use clap::Parser;
use iced_x86::OpKind;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use ropr::{binary::Binary, disassembler::Disassembly};
use rustc_hash::FxHashMap;
use std::{collections::HashSet, path::PathBuf};

use crate::instruction::{
    ImmediateOp::{U8, U16, U32, U64}, Instruction,
};

pub mod constraint;
pub mod instruction;

#[derive(Parser)]
#[clap(version)]
struct Args {
    binary: PathBuf,
}

pub const WEIRD_OP_KINDS: [OpKind; 9] = [
    OpKind::MemoryESDI,
    OpKind::MemorySegSI,
    OpKind::MemorySegDI,
    OpKind::MemoryESEDI,
    OpKind::MemoryESRDI,
    OpKind::MemorySegESI,
    OpKind::MemorySegRSI,
    OpKind::MemorySegEDI,
    OpKind::MemorySegRDI,
];

fn main() {
    let opts = Args::parse();

    let b = opts.binary;
    let b = Binary::new(b).unwrap();
    let sections = b.sections(None).unwrap();

    let gadget_to_addr = sections
        .iter()
        .filter_map(Disassembly::new)
        .flat_map(|dis| {
            (0..dis.bytes().len())
                .into_par_iter()
                .filter(|offset| dis.is_tail_at(*offset, true, false, false, false))
                .flat_map_iter(|tail| dis.gadgets_from_tail(tail, 6, false, true))
                .filter(|gadget| {
                    gadget.0.instructions().iter().all(|i| {
                        !i.op_kinds()
                            .any(|op_kind| WEIRD_OP_KINDS.contains(&op_kind))
                    })
                })
                .map(|gadget| {
                    (
                        gadget
                            .0
                            .instructions()
                            .iter()
                            .map(|i| Instruction::from_iced(*i))
                            .collect::<Vec<_>>(),
                        gadget.0,
                        gadget.1,
                    )
                })
                .map(|gadget| (gadget.1, (gadget.0, gadget.2)))
                .collect::<Vec<_>>()
        })
        .collect::<FxHashMap<_, (_, _)>>();

    let mut gadgets = gadget_to_addr.into_iter().collect::<Vec<_>>();
    gadgets.sort_unstable_by(|(_, (_, addr1)), (_, (_, addr2))| addr1.cmp(addr2));

    let gadget_count = gadgets.len();

    println!("Found {} gadgets..", gadget_count,);
}
