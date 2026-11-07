use std::hash::{Hash, Hasher};

use crate::constraint::{Constraint, TokenResult, elementary};

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct State {
    /// Starting at the last, going towards the first
    pub fullfilled_effects: Vec<TokenResult>,

    /// Starting at the last, going towards the first
    pub wanted_constraints: Vec<Constraint>,
}

impl Hash for State {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.wanted_constraints.hash(state);
    }
}

impl State {
    pub fn initial() -> Self {
        Self {
            fullfilled_effects: Vec::with_capacity(5),
            wanted_constraints: Vec::with_capacity(5),
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
