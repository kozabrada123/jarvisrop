use std::{
    collections::VecDeque,
    hash::{Hash, Hasher},
    sync::Arc,
};

use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use rustc_hash::FxHashSet;

use crate::{
    constraint::{
        Constraint, Token, is_a_subset_of_b, is_constraint_vec_elementary, total_dedup_constraints,
        total_dedup_results,
    },
    gadget::Gadget,
    print_solve,
    state::State,
};

#[derive(Clone, Debug)]
pub struct Searcher {
    pub initial_state: State,
    pub gadgets: Vec<Arc<Gadget>>,

    pub layers: u16,
    pub last_paths: FxHashSet<Path>,
}

impl Searcher {
    pub fn tick(&mut self) {
        self.layers += 1;
        let last = &self.last_paths;

        if last.is_empty() {
            let mut initial = Path::null();
            initial.ending_state = self.initial_state.clone();

            println!(
                "Performing first layer with {} gadgets..",
                self.gadgets.len()
            );

            // first layer
            let c = self
                .gadgets
                .par_iter()
                .filter(|gadget| initial.is_useful_addition(gadget))
                // FIXME: should merge paths with the same contraints and effects but different
                // gadgets
                .map(|g| {
                    let path = initial.clone().with_applied_gadget(g.clone());

                    if let Some(true) =
                        is_constraint_vec_elementary(&path.ending_state.wanted_constraints)
                    {
                        print_solve(path.clone());
                    }

                    path
                })
                .collect::<FxHashSet<_>>();

            self.last_paths = c;
        } else {
            println!("DFS-ing over {} starts..", last.len());

            // DFS!!!
            last.par_iter()
                .filter(|p| !p.ending_state.is_impossible())
                .for_each(|grandma_path| {
                    let mut current_path = grandma_path.clone();
                    let mut parents = Vec::new();

                    'out: loop {
                        // At the end, we are boned if we don't find anything
                        let mut end_of_the_line = true;

                        'gadgets: for gadget in self
                            .gadgets
                            .iter()
                            .filter(|gadget| current_path.is_useful_addition(gadget))
                        {
                            let new_path = current_path.clone().with_applied_gadget(gadget.clone());

                            if let Some(true) = is_constraint_vec_elementary(
                                &new_path.ending_state.wanted_constraints,
                            ) {
                                print_solve(new_path.clone());
                                break 'out;
                            }

                            if parents.len() > 50 {
                                break 'gadgets;
                            }

                            // Go one layer deeper!
                            let new = new_path.ending_state.wanted_constraints.len();
                            let old = current_path.ending_state.wanted_constraints.len();
                            let different = new == old
                                && new_path.ending_state.wanted_constraints
                                    != current_path.ending_state.wanted_constraints;
                            let obj_worse = current_path.is_objectively_worse_than(&new_path);

                            if new < old || different || obj_worse {
                                parents.push(current_path);
                                current_path = new_path;
                                end_of_the_line = false;
                                break 'gadgets;
                            }
                        }

                        if end_of_the_line {
                            break;
                        }
                    }
                });
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Path {
    pub ending_state: State,
    /// Going backwards
    pub executed_gadgets: Vec<Arc<Gadget>>,
}

impl Hash for Path {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.ending_state.hash(state);
    }
}

impl Path {
    pub fn null() -> Self {
        Self {
            ending_state: State::fake(),
            executed_gadgets: vec![],
        }
    }

    pub fn apply_gadget(&mut self, gadget: Arc<Gadget>) {
        let results = gadget.find_results(&mut self.ending_state);

        let needed_constraints = gadget.find_needed_constraints(&mut self.ending_state);
        self.ending_state
            .wanted_constraints
            .append(&mut needed_constraints.into());

        for res in results.iter() {
            let mut insert = false;

            let mut requirements = res.requirements.clone();
            requirements.reverse();

            // check if we are going to get it for free
            // (apply to the current state)
            total_dedup_constraints(&mut requirements, false);

            let final_res = is_constraint_vec_elementary(&requirements);

            if let Some(true) = final_res {
                insert = true;
            }

            if final_res != Some(false) {
                // check if we want it / it fullfills stuff we want
                let mut i = 0;
                while i < self.ending_state.wanted_constraints.len() {
                    let wanted_constraint = self.ending_state.wanted_constraints[i];

                    if wanted_constraint.can_be_transposed_as(res, None) {
                        insert = true;
                        break;
                    }

                    i += 1;
                }
            }

            if insert {
                self.ending_state.fullfilled_effects.push(res.clone());
            }
        }

        total_dedup_results(&mut self.ending_state.fullfilled_effects);
        self.ending_state.recalculate_wanted_constraints();
        self.executed_gadgets.push(gadget.clone());
    }

    pub fn with_applied_gadget(self, gadget: Arc<Gadget>) -> Self {
        let mut s = self;
        s.apply_gadget(gadget);
        s
    }

    /// Returns true if the given path is mathematically worse in every way
    pub fn is_objectively_worse_than(&self, other: &Self) -> bool {
        // Uncomperable
        /*if !is_equal(
            &self.ending_state.fullfilled_effects,
            &other.ending_state.fullfilled_effects,
        ) {
            return false;
        }*/

        // Same results

        // other needs less or equal constraints
        if is_a_subset_of_b(
            &other.ending_state.wanted_constraints,
            &self.ending_state.wanted_constraints,
        ) {
            // They are equal, compare number of gadgets
            if is_a_subset_of_b(
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

    pub fn is_useful_addition(&self, g: &Gadget) -> bool {

        /*for effect in g.results.iter() {
            if self.ending_state.wanted_constraints[0].can_be_transposed_as(effect, None) {
                return true;
            }
        }*/

        for wanted in &self.ending_state.wanted_constraints {
            for effect in g.results.iter() {
                if wanted.can_be_transposed_as(effect, None) {
                    return true;
                }
            }
        }

        false
    }

    pub fn is_impossible(&self) -> bool {
        self.ending_state.is_impossible()
    }
}
