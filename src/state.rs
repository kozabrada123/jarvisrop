use crate::{
    constraint::{Constraint, Token, TokenResult, elementary, total_dedup_constraints},
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
            fullfilled_effects: Vec::with_capacity(15),
            wanted_constraints: Vec::with_capacity(15),
        }
    }

    pub fn with_wanted(self, wanted: Constraint) -> Self {
        let mut s = self;

        if !s.wanted_constraints.contains(&wanted) {
            s.wanted_constraints.push(wanted);
        }

        s
    }

    pub fn is_impossible(&self) -> bool {
        for wanted in &self.wanted_constraints {
            if wanted == &elementary(false) {
                return true;
            }
        }

        return false;
    }
}
