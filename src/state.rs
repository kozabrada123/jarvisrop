use std::{
    collections::VecDeque,
    hash::{Hash, Hasher},
    sync::Arc,
};

use crate::constraint::{
    Constraint, ElementaryConstraint, TokenResult, UnaryOperator::IsUnchanged, elementary,
    total_dedup_constraints, unary,
};

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct State {
    /// Starting at the last, going towards the first
    pub fullfilled_effects: Vec<TokenResult>,

    /// Starting at the last, going towards the first
    pub wanted_constraints: Vec<Constraint>,

    /// Our original goal
    pub original_wanted_constraints: Arc<Vec<Constraint>>,
}

impl Hash for State {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.wanted_constraints.hash(state);
    }
}

impl State {
    pub fn fake() -> Self {
        Self {
            fullfilled_effects: Vec::new(),
            wanted_constraints: Vec::new(),
            original_wanted_constraints: Arc::new(Vec::new()),
        }
    }

    pub fn initial(wanted: Arc<Vec<Constraint>>) -> Self {
        Self {
            fullfilled_effects: Vec::with_capacity(5),
            wanted_constraints: wanted.iter().cloned().collect(),
            original_wanted_constraints: wanted,
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

    /// Recalcultess which constraints are still needed
    /// based off the executed effects
    pub fn recalculate_wanted_constraints(&mut self) {
        // Read the originals we thought we fulfilled
        self.wanted_constraints.reverse();

        for orig in self.original_wanted_constraints.iter().rev() {
            if !self.wanted_constraints.contains(orig) {
                self.wanted_constraints.push(*orig);
            }
        }

        self.wanted_constraints.reverse();

        // Go through our effects and see which ones we actually satisfied
        for res in self.fullfilled_effects.iter() {
            let mut i = 0;
            while i < self.wanted_constraints.len() {
                let wanted_constraint = self.wanted_constraints[i];

                if let Constraint::Elementary(ElementaryConstraint(b)) = wanted_constraint {
                    if b {
                        self.wanted_constraints.remove(i);
                        continue;
                    } else {
                        self.wanted_constraints = vec![wanted_constraint];
                        return;
                    }
                }

                let mut new_constraints = res.requirements.clone();
                new_constraints.reverse();

                // We know we're gonna add it
                if wanted_constraint.can_be_transposed_as(res, Some(&mut new_constraints)) {
                    self.wanted_constraints.remove(i);
                    self.wanted_constraints.append(&mut new_constraints);
                    continue;
                }

                self.wanted_constraints.append(&mut new_constraints);

                i += 1;
            }
        }

        // Clean up any unchanged constraints that were left over
        // TODO: more proper handling of these
        let mut i = 0;
        while i < self.wanted_constraints.len() {
            let wanted_constraint = self.wanted_constraints[i];

            match wanted_constraint {
                Constraint::Unary(uc) => match uc.operator {
                    IsUnchanged => {
                        // Check for any conflicting ones "later" (earlier) on
                        let mut j = i + 1;
                        while j < self.wanted_constraints.len() {
                            let con_2 = self.wanted_constraints[j];

                            if con_2.overwrites(&wanted_constraint) {
                                self.wanted_constraints = vec![elementary(false)];
                                return;
                            }

                            j += 1;
                        }

                        self.wanted_constraints.remove(i);
                        continue;
                    }
                    _ => {}
                },
                _ => {}
            }

            i += 1;
        }

        total_dedup_constraints(&mut self.wanted_constraints, false);
    }
}
