use clap::Parser;
use iced_x86::{
    OpKind, Register::{R14, RAX, RCX, RDI},
};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use ropr::{binary::Binary, disassembler::Disassembly};
use rustc_hash::FxHashMap;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
};

use crate::{
    constraint::{binary, unary},
    instruction::{
        ImmediateOp::{U8, U16, U32, U64},
        Instruction, Operand,
    },
    search::{Path, Searcher},
    state::State,
};

pub mod constraint;
pub mod gadget;
pub mod hash_util;
pub mod instruction;
pub mod search;
pub mod state;

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
                        crate::gadget::Gadget::from_inner(
                            Arc::new(gadget.0),
                            &mut State::initial(),
                        ),
                        gadget.1,
                    )
                })
                .map(|gadget| (gadget.1, gadget.0))
                .collect::<Vec<_>>()
        })
        .collect::<FxHashMap<_, _>>();

    let mut gadgets = gadget_to_addr.into_iter().collect::<Vec<_>>();
    gadgets.sort_unstable_by(|(addr1, _), (addr2, _)| addr1.cmp(addr2));

    let gadget_count = gadgets.len();

    println!("Found {} gadgets..", gadget_count,);

    let mut searcher = Searcher {
        gadgets: gadgets.into_iter().map(|(id, gadget)| gadget).collect(),
        wanted: binary(
            Operand::Register(RAX),
            constraint::BinaryOperator::IsEqual,
            Operand::Register(RDI)
        ),
        initial_state: State::initial(),
        found_solve: (0, Path::null()),

        found_paths: vec![],
    };

    let mut num_searches = 0;

    while num_searches < 10 && searcher.found_solve.0 == 0 {
        println!(
            "Tick {}: {} - {}",
            num_searches,
            searcher.found_paths.len(),
            searcher.found_paths.last().unwrap_or(&HashMap::new()).len()
        );
        searcher.tick();
        num_searches += 1;
    }

    println!("Found chain! {:?}", searcher.found_solve.1); 
}
