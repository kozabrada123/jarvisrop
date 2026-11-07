use clap::Parser;
use iced_x86::{OpKind, Register::*};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use ropr::{binary::Binary, disassembler::Disassembly};
use rustc_hash::FxHashMap;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
};

use crate::{
    constraint::{
        BinaryOperator::IsEqual,
        UnaryOperator::{self, IsControlled},
        a_part_of_b, binary, unary,
    },
    instruction::{
        ImmediateOp::{U8, U16, U32, U64},
        Instruction,
        Operand::{self, Register},
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

pub const WEIRD_OP_KINDS: [OpKind; 10] = [
    OpKind::Memory, // Temporary optimization until we support it!
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
    let sections = b.sections(Option::None).unwrap();

    let gadgets = sections
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
                    (crate::gadget::Gadget::from_inner(
                        Arc::new(gadget.0),
                        &mut State::initial(),
                    ),)
                })
                .map(|gadget| (gadget.0.results.len(), gadget.0))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<(_, _)>>();

    let mut gadgets = gadgets.into_iter().collect::<Vec<_>>();
    gadgets.sort_unstable_by(|(complexity1, _), (complexity2, _)| complexity1.cmp(complexity2));

    let gadget_count = gadgets.len();

    println!("Found {} gadgets..", gadget_count,);

    let mut searcher = Searcher {
        gadgets: gadgets.into_iter().map(|(_id, gadget)| gadget).collect(),
        wanted: vec![
            binary(Register(RAX), IsEqual, Register(RCX)),
            unary(Register(RSI), IsControlled),
            unary(Register(RDI), IsControlled),
        ],
        initial_state: State::initial(),
        found_paths: vec![],
    };

    let mut num_searches = 0;

    while num_searches < 6 {
        println!(
            "Tick {}: {} - {}",
            num_searches,
            searcher.found_paths.len(),
            searcher.found_paths.last().unwrap_or(&HashSet::new()).len()
        );
        searcher.tick();
        num_searches += 1;
    }

    println!("No solve found :(");
}

pub fn print_solve(chain: Path) {

    let mut solve_str = String::with_capacity(128);

    solve_str.push_str("\nFound chain!\n");
    solve_str.push_str("End state:\n");
    solve_str.push_str("- constraints:\n");
    for constraint in chain.ending_state.wanted_constraints {
        solve_str.push_str(&format!("  - {:?}\n", constraint));
    }
    solve_str.push_str("\n- fullfilled effects:\n");
    for effect in chain.ending_state.fullfilled_effects {
        solve_str.push_str(&format!("  - {:?}\n", effect));
    }
    solve_str.push_str("\nGadgets:\n");
    for gadget in chain.executed_gadgets.iter().rev() {
        solve_str.push_str(&format!("- {:?}\n", gadget));
    }
    solve_str.push_str("\n");

    print!("{}", solve_str);

    std::process::exit(0);
}
