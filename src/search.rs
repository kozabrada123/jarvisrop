use std::collections::HashMap;

use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

use crate::{
    constraint::{
        Constraint, Token, elementary, is_constraint_vec_elementary, is_equal, is_subset,
        total_dedup_constraints,
    },
    gadget::Gadget,
    hash_util::compute_hash,
    state::State,
};

#[derive(Clone, Debug)]
pub struct Searcher {
    pub initial_state: State,
    pub wanted: Constraint,
    pub gadgets: Vec<Gadget>,

    pub found_paths: Vec<HashMap<u64, Path>>,
    pub found_solve: (u64, Path),
}

impl Searcher {
    pub fn tick(&mut self) {
        let last = self.found_paths.last();

        if last.is_none() {
            let mut initial = Path::null();
            initial.ending_state = self.initial_state.clone().with_wanted(self.wanted.clone());

            println!(
                "Performing first layer with {} gadgets..",
                self.gadgets.len()
            );

            // first layer
            let found_paths = self
                .gadgets
                .par_iter()
                .filter(|gadget| initial.is_useful_addition(&mut (*gadget).clone()))
                // FIXME: should merge paths with the same contraints and effects but different
                // gadgets
                .map(|g| Path::null().with_applied_gadget(g))
                .map(|x| (compute_hash(&x), x))
                .collect::<HashMap<_, _>>();

            found_paths.iter().for_each(|(id, p)| {
                if let Some(true) = is_constraint_vec_elementary(&p.ending_state.wanted_constraints)
                {
                    self.found_solve = (*id, p.clone());
                    return;
                }
            });

            self.found_paths.push(found_paths);
        } else {
            let last_u = last.unwrap();

            let mut this_layer = HashMap::new();

            last_u
                .iter()
                .filter(|p| !p.1.ending_state.is_impossible())
                .filter(|p| p.1.ending_state.wanted_constraints.len() < 3)
                .for_each(|(id, path)| {
                    let found_developments = self
                        .gadgets
                        .par_iter()
                        .filter(|gadget| path.is_useful_addition(&mut (*gadget).clone()))
                        .map(|g| path.clone().with_applied_gadget_and_previous(g, *id))
                        .map(|x| (compute_hash(&x), x))
                        .collect::<HashMap<_, _>>();

                    found_developments.into_iter().for_each(|(id, p)| {
                        if let Some(true) =
                            is_constraint_vec_elementary(&p.ending_state.wanted_constraints)
                        {
                            self.found_solve = (id, p.clone());
                            return;
                        }

                        this_layer.insert(id, p);
                    });
                });
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Path {
    pub ending_state: State,
    pub executed_gadgets: Vec<Gadget>,
    pub previous_hash: u64,
}

impl Path {
    pub fn null() -> Self {
        Self {
            ending_state: State::initial(),
            executed_gadgets: vec![],
            previous_hash: 0,
        }
    }

    pub fn apply_gadget(&mut self, gadget: &Gadget) {
        let mut results = gadget.find_results(&mut self.ending_state);

        let mut wanted_constraints = self.ending_state.wanted_constraints.clone();

        wanted_constraints.append(&mut gadget.find_needed_constraints(&mut self.ending_state));

        for res in results.iter_mut() {
            let mut i = 0;

            let mut insert = false;

            let mut requirements = res.requirements.clone();

            // check if we are going to get it for free
            // (apply to the current state)
            requirements = total_dedup_constraints(requirements, &mut self.ending_state);

            let final_res = is_constraint_vec_elementary(&requirements);

            if let Some(true) = final_res {
                insert = true;
            }

            if final_res != Some(false) {
                // check if we want it / it fullfills stuff we want
                while i < wanted_constraints.len() {
                    let wanted_constraint = wanted_constraints[i];

                    if wanted_constraint.can_be_transposed_as(res) {
                        wanted_constraints.remove(i);
                        insert = true;
                        println!("new: {:?}", res.requirements);
                        continue;
                    }

                    i += 1;
                }
            }

            if insert {
                wanted_constraints.append(&mut requirements);
            }
        }

        wanted_constraints = total_dedup_constraints(wanted_constraints, &mut self.ending_state);

        let mut gadget_results = gadget.find_results(&mut self.ending_state);
        self.ending_state
            .fullfilled_effects
            .append(&mut gadget_results);

        self.ending_state.wanted_constraints = wanted_constraints;

        self.executed_gadgets.push(gadget.clone());
    }

    pub fn with_applied_gadget(self, gadget: &Gadget) -> Self {
        let mut s = self;
        s.apply_gadget(gadget);
        s
    }

    pub fn with_applied_gadget_and_previous(self, gadget: &Gadget, previous: u64) -> Self {
        let mut s = self;
        s.apply_gadget(gadget);
        s.previous_hash = previous;
        s
    }

    /// Returns true if the given path is mathematically worse in every way
    pub fn is_objectively_worse_than(&self, other: &Self) -> bool {
        // Uncomperable
        if !is_equal(
            &self.ending_state.fullfilled_effects,
            &other.ending_state.fullfilled_effects,
        ) {
            return false;
        }

        // Same results

        // other needs less or equal constraints
        if is_subset(
            &other.ending_state.wanted_constraints,
            &self.ending_state.wanted_constraints,
        ) {
            // They are equal, compare number of gadgets
            if is_subset(
                &self.ending_state.wanted_constraints,
                &other.ending_state.wanted_constraints,
            ) {
                return self.executed_gadgets.len() > other.executed_gadgets.len();
            }

            // They are not equal, other has less and does the same
            return true;
        }

        return false;
    }

    pub fn is_useful_addition(&self, g: &mut Gadget) -> bool {
        let mut useful = false;
        for wanted in &self.ending_state.wanted_constraints {
            for effect in g.results.iter_mut() {
                if wanted.can_be_transposed_as(effect) {
                    useful = true;
                }
            }
        }

        useful
    }

    pub fn is_impossible(&self) -> bool {
        self.ending_state.is_impossible()
    }
}
