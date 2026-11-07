use std::collections::HashMap;

use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

use crate::{
    constraint::{Constraint, elementary},
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
            // first layer
            let found_paths = self
                .gadgets
                .par_iter()
                .filter(|x| x.results.iter().any(|y| y.effects.contains(&self.wanted))) // <- ;-;
                // the contains wanted needs to be a wildcard search thingy
                .map(|g| Path {
                    executed_gadgets: vec![g.clone()],
                    ending_state: self.initial_state.clone().with_applied_gadget(g),
                    previous_hash: 0,
                })
                .map(|x| (compute_hash(&x), x))
                .collect::<HashMap<_, _>>();

            println!("bleh {}", found_paths.len());
            self.found_paths.push(found_paths);
        } else {
            let last_u = last.unwrap();

            let mut this_layer = HashMap::new();

            last_u
                .iter()
                .filter(|p| p.1.ending_state.wanted_constraints.len() < 3)
                .for_each(|(id, path)| {
                    let found_developments = self
                        .gadgets
                        .par_iter()
                        .filter(|x| {
                            x.results.par_iter().any(|y| {
                                path.ending_state
                                    .wanted_constraints
                                    .iter()
                                    .any(|wc| y.effects.contains(&wc))
                            })
                        })
                        .map(|g| Path {
                            executed_gadgets: [path.executed_gadgets.clone(), vec![g.clone()]]
                                .concat(),
                            ending_state: path.ending_state.clone().with_applied_gadget(g),
                            previous_hash: *id,
                        })
                        .map(|x| (compute_hash(&x), x))
                        .collect::<HashMap<_, _>>();

                    found_developments.into_iter().for_each(|(id, p)| {
                        if p.ending_state.wanted_constraints.len() == 0
                            || (p.ending_state.wanted_constraints.len() == 1
                                && p.ending_state.wanted_constraints[0] == elementary(true))
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
        Self { ending_state: State::initial(), executed_gadgets: vec![], previous_hash: 0 } 
    }
}
