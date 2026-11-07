use crate::{
    constraint::{Constraint, Token, TokenResult, total_dedup_constraints},
    gadget::Gadget,
};

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct State {
    pub fullfilled_effects: Vec<TokenResult>,
    pub wanted_constraints: Vec<Constraint>,
}

impl State {
    pub fn initial() -> Self {
        Self {
            fullfilled_effects: Vec::new(),
            wanted_constraints: Vec::new(),
        }
    }

    pub fn apply_gadget(&mut self, gadget: &Gadget) {
        let results = gadget.find_results();

        let mut wanted_constraints = self.wanted_constraints.clone();

        wanted_constraints.append(&mut gadget.find_needed_constraints());

        for res in results {
            let mut i = 0;
            while i < wanted_constraints.len() {
                let wanted_constraint = wanted_constraints[i];

                if res.effects.contains(&wanted_constraint) {
                    wanted_constraints.remove(i);
                    wanted_constraints.append(&mut res.requirements.clone());
                    continue;
                }

                i += 1;
            }
        }

        wanted_constraints = total_dedup_constraints(wanted_constraints);

        self.fullfilled_effects.append(&mut gadget.find_results());
        self.wanted_constraints = wanted_constraints;
    }

    pub fn with_applied_gadget(self, gadget: &Gadget) -> Self {
        let mut s = self;
        s.apply_gadget(gadget);
        s
    }
}
