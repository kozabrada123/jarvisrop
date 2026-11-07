use crate::{
    constraint::{Constraint, Token, TokenResult, elementary, total_dedup_constraints},
    gadget::Gadget,
};

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct State {
    /// Starting at the last, going towards the first
    pub fullfilled_effects: Vec<TokenResult>,

    /// Starting at the last, going towards the first
    pub wanted_constraints: Vec<Constraint>,
}

impl State {
    pub fn initial() -> Self {
        Self {
            fullfilled_effects: Vec::with_capacity(15),
            wanted_constraints: Vec::with_capacity(15),
        }
    }

    pub fn with_wanted(self, wanted: Vec<Constraint>) -> Self {
        let mut s = self;

        for c in wanted {
            if !s.wanted_constraints.contains(&c) {
                s.wanted_constraints.push(c);
            }
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
