use std::{
    collections::{HashMap, HashSet},
    thread,
};

use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

use crate::{
    constraint::{
        Constraint, Token, elementary, is_a_subset_of_b, is_constraint_vec_elementary, is_equal,
        total_dedup_constraints,
    },
    gadget::Gadget,
    hash_util::compute_hash,
    print_solve,
    state::State,
};

#[derive(Clone, Debug)]
pub struct Searcher {
    pub initial_state: State,
    pub wanted: Vec<Constraint>,
    pub gadgets: Vec<Gadget>,

    pub found_paths: Vec<HashSet<Path>>,
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
            let c = self
                .gadgets
                .par_iter()
                .filter(|gadget| initial.is_useful_addition(gadget).clone())
                // FIXME: should merge paths with the same contraints and effects but different
                // gadgets
                .map(|g| {
                    let path = initial.clone().with_applied_gadget(g);

                    if let Some(true) =
                        is_constraint_vec_elementary(&path.ending_state.wanted_constraints)
                    {
                        print_solve(path.clone());
                    }

                    path
                })
                .collect::<HashSet<_>>();

            self.found_paths.push(c);
        } else {
            let last_u = last.unwrap();

            let mut this_layer = HashSet::new();

            last_u
                .iter()
                .filter(|p| !p.ending_state.is_impossible())
                .filter(|p| p.ending_state.wanted_constraints.len() < (2 + self.wanted.len()))
                .for_each(|path| {
                    let found_developments = self
                        .gadgets
                        .par_iter()
                        .map(|gadget| {
                            let new_path = path.clone().with_applied_gadget(gadget);

                            if let Some(true) = is_constraint_vec_elementary(
                                &new_path.ending_state.wanted_constraints,
                            ) {
                                print_solve(new_path.clone());
                            }

                            new_path
                        })
                        .filter(|new_path| !new_path.is_objectively_worse_than(path))
                        .collect::<HashSet<_>>();

                    for found in found_developments {
                        this_layer.insert(found);
                    }
                });
            self.found_paths.push(this_layer);
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Path {
    pub ending_state: State,
    /// Going backwards
    pub executed_gadgets: Vec<Gadget>,
}

impl Path {
    pub fn null() -> Self {
        Self {
            ending_state: State::initial(),
            executed_gadgets: vec![],
        }
    }

    pub fn apply_gadget(&mut self, gadget: &Gadget) {
        let results = gadget.find_results(&mut self.ending_state);

        let mut wanted_constraints = self.ending_state.wanted_constraints.clone();
        //println!("{:?} | wanted_constraints: {:?}", thread::current().id(), wanted_constraints);

        wanted_constraints.append(&mut gadget.find_needed_constraints(&mut self.ending_state));

        //println!("{:?} | wanted_constraints + gadget needed: {:?}", thread::current().id(), wanted_constraints);

        for res in results.iter() {
            let mut i = 0;

            let mut insert = false;

            let mut requirements = res.requirements.clone();
            requirements.reverse();

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

                    if wanted_constraint.can_be_transposed_as(res, &mut requirements, false) {
                        wanted_constraints.remove(i);
                        insert = true;
                        continue;
                    }

                    i += 1;
                }
            }

            //println!("{:?} | wanted_constraints after res: {:?} ({:?})", thread::current().id(), wanted_constraints, res);

            if insert {
                wanted_constraints.append(&mut requirements);
                self.ending_state.fullfilled_effects.push(res.clone());
            }

            /*println!(
                "{:?} | wanted_constraints after res + req: {:?}",
                thread::current().id(),
                wanted_constraints
            );*/
        }

        //println!("{:?} | wanted_constraints predup: {:?}", thread::current().id(), wanted_constraints);
        wanted_constraints = total_dedup_constraints(wanted_constraints, &mut self.ending_state);
        //println!("{:?} | wanted_constraints postdup: {:?}", thread::current().id(), wanted_constraints);

        self.ending_state.wanted_constraints = wanted_constraints;

        self.executed_gadgets.push(gadget.clone());

        /*if is_constraint_vec_elementary(&self.ending_state.wanted_constraints) == Some(true) {
            println!("{:?} - {:?}", thread::current().id(), self);
            std::process::exit(0);
        }*/
    }

    pub fn with_applied_gadget(self, gadget: &Gadget) -> Self {
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
        for wanted in &self.ending_state.wanted_constraints {
            for effect in g.results.iter() {
                if wanted.can_be_transposed_as(effect, &mut Vec::new(), true) {
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
