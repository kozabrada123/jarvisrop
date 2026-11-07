use std::{
    fmt::{self, Write},
    sync::Arc,
};

use crate::{
    constraint::{
        Token, dedup_constraints, dedup_results, total_dedup_constraints, total_dedup_results,
    },
    hash_util::compute_hash,
    instruction::Instruction,
    state::State,
};

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Gadget {
    pub inner: Arc<ropr::gadgets::Gadget>,
    pub instructions: Vec<Instruction>,

    /// from the first result seen to the last
    pub results: Vec<crate::constraint::TokenResult>,

    /// from the last constraint needed to the first
    pub constraints: Vec<crate::constraint::Constraint>,
}

impl fmt::Debug for Gadget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut instructions = String::new();

        for ins in &self.instructions {
            instructions.push_str(&ins.inner.to_string());
            instructions.push_str("; ");
        }

        f.write_fmt(format_args!(
            "G({} ({:?} | {:?}))",
            instructions, self.constraints, self.results
        ))
    }
}

impl Token for Gadget {
    // FIXME: is it really that different based on state?
    fn find_results(&self, _state: &mut State) -> Vec<crate::constraint::TokenResult> {
        self.results.clone()
    }

    fn find_needed_constraints(&self, _state: &mut State) -> Vec<crate::constraint::Constraint> {
        self.constraints.clone()
    }
}

impl Gadget {
    pub fn from_inner(inner: Arc<ropr::gadgets::Gadget>, state: &mut State) -> Self {
        let instructions = inner
            .instructions()
            .iter()
            .map(|i| Instruction::from_iced(*i))
            .collect::<Vec<_>>();

        let results = Self::calculate_results(&instructions, state);
        let constraints = Self::calculate_needed_constraints(&instructions, state);

        Self {
            inner,
            instructions,
            results,
            constraints,
        }
    }

    fn calculate_needed_constraints(
        is: &Vec<Instruction>,
        state: &mut State,
    ) -> Vec<crate::constraint::Constraint> {
        let mut cs = Vec::with_capacity(5 * is.len());

        for ins in is.iter().rev() {
            cs.append(&mut total_dedup_constraints(
                ins.find_needed_constraints(state),
                state,
            ));
        }

        // Make later constraints earlier
        cs.reverse();
        cs = total_dedup_constraints(cs, state);

        return cs;
    }

    fn calculate_results(
        is: &Vec<Instruction>,
        state: &mut State,
    ) -> Vec<crate::constraint::TokenResult> {
        let mut resus = Vec::with_capacity(5 * is.len());

        for ins in is {
            resus.append(&mut total_dedup_results(ins.find_results(state)));
        }

        // Make later results earlier
        resus.reverse();
        resus = total_dedup_results(resus);

        return resus;
    }
}
