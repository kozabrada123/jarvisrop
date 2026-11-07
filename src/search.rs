use std::{
    collections::HashSet,
    hash::{Hash, Hasher},
    sync::Arc,
};

use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use rustc_hash::FxHashSet;

use crate::{
    constraint::{
        Constraint, Token, is_a_subset_of_b, is_constraint_vec_elementary, total_dedup_constraints,
        total_dedup_constraints_for_state,
    },
    gadget::Gadget,
    print_solve,
    state::State,
};

#[derive(Clone, Debug)]
pub struct Searcher {
    pub initial_state: State,
    pub wanted: Vec<Constraint>,
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
            initial.ending_state = self.initial_state.clone().with_wanted(self.wanted.clone());

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
            let mut this_layer = FxHashSet::default();

            last.iter()
                .filter(|p| !p.ending_state.is_impossible())
                .filter(|p| p.ending_state.wanted_constraints.len() < (2 + self.wanted.len()))
                .for_each(|path| {
                    let found_developments = self
                        .gadgets
                        .par_iter()
                        .map(|gadget| {
                            let new_path = path.clone().with_applied_gadget(gadget.clone());

                            if let Some(true) = is_constraint_vec_elementary(
                                &new_path.ending_state.wanted_constraints,
                            ) {
                                print_solve(new_path.clone());
                            }

                            new_path
                        })
                        .filter(|new_path| !new_path.is_objectively_worse_than(path))
                        .collect::<FxHashSet<_>>();

                    for found in found_developments {
                        this_layer.insert(found);
                    }
                });

            self.last_paths = this_layer;
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
            ending_state: State::initial(),
            executed_gadgets: vec![],
        }
    }

    pub fn apply_gadget(&mut self, gadget: Arc<Gadget>) {
        let results = gadget.find_results(&mut self.ending_state);

        //println!("{:?} | wanted_constraints: {:?}", thread::current().id(), wanted_constraints);

        let mut needed_constraints = gadget.find_needed_constraints(&mut self.ending_state);
        self.ending_state
            .wanted_constraints
            .append(&mut needed_constraints);

        //println!("{:?} | wanted_constraints + gadget needed: {:?}", thread::current().id(), wanted_constraints);

        for res in results.iter() {
            let mut i = 0;

            let mut insert = false;

            let mut requirements = res.requirements.clone();
            requirements.reverse();

            // check if we are going to get it for free
            // (apply to the current state)
            total_dedup_constraints(&mut requirements);

            let final_res = is_constraint_vec_elementary(&requirements);

            if let Some(true) = final_res {
                insert = true;
            }

            if final_res != Some(false) {
                // check if we want it / it fullfills stuff we want
                while i < self.ending_state.wanted_constraints.len() {
                    let wanted_constraint = self.ending_state.wanted_constraints[i];

                    if wanted_constraint.can_be_transposed_as(res, &mut requirements, false) {
                        self.ending_state.wanted_constraints.remove(i);
                        insert = true;
                        continue;
                    }

                    i += 1;
                }
            }

            //println!("{:?} | wanted_constraints after res: {:?} ({:?})", thread::current().id(), wanted_constraints, res);

            if insert {
                self.ending_state
                    .wanted_constraints
                    .append(&mut requirements);
                self.ending_state.fullfilled_effects.push(res.clone());
            }

            /*println!(
                "{:?} | wanted_constraints after res + req: {:?}",
                thread::current().id(),
                wanted_constraints
            );*/
        }

        //println!("{:?} | wanted_constraints predup: {:?}", thread::current().id(), wanted_constraints);
        total_dedup_constraints_for_state(&mut self.ending_state);
        //println!("{:?} | wanted_constraints postdup: {:?}", thread::current().id(), wanted_constraints);

        self.executed_gadgets.push(gadget.clone());

        /*if is_constraint_vec_elementary(&self.ending_state.wanted_constraints) == Some(true) {
            println!("{:?} - {:?}", thread::current().id(), self);
            std::process::exit(0);
        }*/
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
