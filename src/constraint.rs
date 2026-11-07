use std::{collections::HashSet, fmt};

use iced_x86::Register;

use crate::{
    constraint::Constraint::Unary,
    hash_util::compute_hash,
    instruction::{Instruction, Operand},
    state::State,
};

pub trait Token {
    /// Find the constraints the token needs to execute
    fn find_needed_constraints(&self, state: &mut State) -> Vec<Constraint>;

    /// Find the results that this token has
    fn find_results(&self, state: &mut State) -> Vec<TokenResult>;
}

impl Token for Instruction {
    fn find_needed_constraints(&self, state: &mut State) -> Vec<Constraint> {
        match self.inner.mnemonic() {
            iced_x86::Mnemonic::Mov => {
                let dst = self.operands[0].unwrap();
                let src = self.operands[1].unwrap();

                let mut c = Vec::new();

                match dst {
                    Operand::Immediate(_) => {
                        return vec![elementary(false)];
                    }
                    Operand::Memory(_) => {
                        c.push(unary(dst, UnaryOperator::IsWritableAddr));
                    }
                    Operand::Register(_) => {}
                }

                match src {
                    Operand::Memory(_) => {
                        c.push(unary(dst, UnaryOperator::IsWritableAddr));
                    }
                    _ => {}
                }

                return c;
            }
            // TODO:
            _ => vec![elementary(false)],
        }
    }

    fn find_results(&self, state: &mut State) -> Vec<TokenResult> {
        match self.inner.mnemonic() {
            iced_x86::Mnemonic::Mov => {
                let dst = self.operands[0].unwrap();
                let src = self.operands[1].unwrap();

                let requirements = vec![unary(src, UnaryOperator::IsControlled)];
                let effects = vec![binary(dst, BinaryOperator::IsEqual, src)];

                return vec![TokenResult {
                    requirements,
                    effects,
                }];
            }
            iced_x86::Mnemonic::Pop => {
                let dst = self.operands[0].unwrap();

                let requirements = vec![elementary(true)];
                let effects = vec![unary(dst, UnaryOperator::IsControlled)];

                return vec![TokenResult {
                    requirements,
                    effects,
                }];
            }
            // TODO:
            _ => vec![],
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct TokenResult {
    pub requirements: Vec<Constraint>,
    pub effects: Vec<Constraint>,
}

impl fmt::Debug for TokenResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
       f.write_fmt(format_args!("R({:?} -> {:?})", self.requirements, self.effects)) 
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Constraint {
    Binary(BinaryConstraint),
    Unary(UnaryConstraint),
    Elementary(ElementaryConstraint),
}

impl fmt::Debug for Constraint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unary(u) => f.write_fmt(format_args!("{:?}", u)),
            Self::Binary(b) => f.write_fmt(format_args!("{:?}", b)),
            Self::Elementary(e) => f.write_fmt(format_args!("{:?}", e)),
        }
    }
}

impl Constraint {
    /// Convert self into a list of constraints self depends on
    ///
    /// self.flatten => self
    pub fn flatten(self, state: &mut State) -> Vec<Constraint> {
        match self {
            Constraint::Elementary(_e) => vec![self],
            Constraint::Unary(u) => u.flatten(state),
            Constraint::Binary(b) => b.flatten(state),
        }
    }

    /// Returns true if other is a "transposition" of self
    ///
    /// For example, RAX is controlled can be rewritten as RAX = RDI where RDI is controlled
    pub fn can_be_transposed_as(self, other: &mut TokenResult) -> bool {

        // Word for word the same
        if other.effects.contains(&self) {
            return true;
        }

        match self {
            Self::Unary(u) => match u.operator {
                // Is controlled can be written as equals something else that is controlled
                UnaryOperator::IsControlled => {
                    for effect in &other.effects {
                        match effect {
                            Self::Binary(b) => match b.operator {
                                BinaryOperator::IsEqual => {
                                    if b.target == u.target {
                                        other.requirements.push(unary(b.wanted_result, u.operator));
                                        return true;
                                    }
                                }
                                _ => {}
                            },
                            _ => {}
                        }
                    }

                    false
                }
                _ => false,
            },
            Self::Binary(b) => match b.operator {
                // TODO: rewrite is not equal to is equal / is lower / is greater than something
                // else
                BinaryOperator::IsEqual => {
                    for effect in &other.effects {
                        match effect {
                            Self::Binary(o) => match o.operator {
                                BinaryOperator::IsEqual => {
                                    if b.target == o.target {
                                        other.requirements.push(binary(o.wanted_result, b.operator, b.wanted_result));
                                        return true;
                                    }
                                }
                                _ => {}
                            },
                            _ => {}
                        }
                    }

                    false
                }
                _ => false,
            },
            Self::Elementary(ElementaryConstraint(always_value)) => {
                return always_value;
            }
            _ => false,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct BinaryConstraint {
    pub target: Operand,
    pub operator: BinaryOperator,
    pub wanted_result: Operand,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum BinaryOperator {
    IsEqual,
    IsNotEqual,
    IsGreater,
    IsLess,
}

impl fmt::Debug for BinaryConstraint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
       f.write_fmt(format_args!("C({:?} {:?} {:?})", self.target, self.operator, self.wanted_result)) 
    }
}

impl BinaryConstraint {
    /// Convert self into a list of constraints self depends on
    ///
    /// self.flatten => self
    pub fn flatten(self, state: &mut State) -> Vec<Constraint> {
        if self.target == self.wanted_result {
            if self.operator == BinaryOperator::IsEqual {
                return vec![elementary(true)];
            }

            return vec![elementary(false)];
        }

        match self.target {
            Operand::Memory(_memory) => {
                return vec![Constraint::Unary(UnaryConstraint {
                    target: self.target,
                    operator: UnaryOperator::IsControlled,
                })];
            }

            Operand::Register(reg) => {
                return vec![Constraint::Unary(UnaryConstraint {
                    target: self.target,
                    operator: UnaryOperator::IsControlled,
                })];
            }

            Operand::Immediate(i) => match self.target {
                Operand::Immediate(j) => match self.operator {
                    BinaryOperator::IsEqual => vec![elementary(i.to_u64().eq(&j.to_u64()))],
                    BinaryOperator::IsNotEqual => vec![elementary(i.to_u64().ne(&j.to_u64()))],
                    BinaryOperator::IsGreater => vec![elementary(i.to_u64().gt(&j.to_u64()))],
                    BinaryOperator::IsLess => vec![elementary(i.to_u64().lt(&j.to_u64()))],
                },
                _ => {
                    // FIXME: another thing of definitions, we could reply with just "no" instead
                    return vec![Constraint::Binary(Self {
                        target: self.wanted_result,
                        operator: self.operator,
                        wanted_result: self.target,
                    })];
                }
            },
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct UnaryConstraint {
    pub target: Operand,
    pub operator: UnaryOperator,
}

impl fmt::Debug for UnaryConstraint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
       f.write_fmt(format_args!("C({:?} {:?})", self.target, self.operator)) 
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum UnaryOperator {
    IsWritableAddr,
    IsReadableAddr,
    IsExecutableAddr,

    IsControlled,
}

impl UnaryConstraint {
    /// Convert self into a list of constraints self depends on
    ///
    /// self.flatten => self
    pub fn flatten(self, state: &mut State) -> Vec<Constraint> {
        match self.target {
            Operand::Memory(_memory) => {
                match self.operator {
                    UnaryOperator::IsControlled => {
                        // FIXME: should be actually flattened, seen via context and some clever
                        // unpacking to break it into its individual components
                        vec![Constraint::Unary(self)]
                    }

                    // After it is dereferenced...
                    UnaryOperator::IsWritableAddr
                    | UnaryOperator::IsExecutableAddr
                    | UnaryOperator::IsReadableAddr => {
                        vec![Constraint::Unary(Self {
                            target: self.target,
                            operator: UnaryOperator::IsControlled,
                        })]
                    }
                }
            }

            Operand::Register(reg) => {
                match self.operator {
                    UnaryOperator::IsControlled => {
                        if reg == Register::BP || reg == Register::RBP {
                            return vec![elementary(true)];
                        }

                        // FIXME: depends on how we want to handle this tbh, as true or false
                        if reg == Register::None {
                            return vec![elementary(true)];
                        }

                        vec![Constraint::Unary(self)]
                    }

                    UnaryOperator::IsWritableAddr
                    | UnaryOperator::IsExecutableAddr
                    | UnaryOperator::IsReadableAddr => {
                        vec![Constraint::Unary(Self {
                            target: self.target,
                            operator: UnaryOperator::IsControlled,
                        })]
                    }
                }
            }

            Operand::Immediate(_i) => {
                match self.operator {
                    // Dumb, will never be true
                    UnaryOperator::IsControlled => {
                        vec![elementary(false)]
                    }

                    // FIXME: pie, whatever, ... context reliant!
                    UnaryOperator::IsWritableAddr
                    | UnaryOperator::IsExecutableAddr
                    | UnaryOperator::IsReadableAddr => {
                        vec![elementary(false)]
                    }
                }
            }
        }
    }
}

/// ElementaryConstraint(true) is always true, such as 1 = 1
/// ElementaryConstraint(false) is always false, such as 1 = 0
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ElementaryConstraint(bool);

impl fmt::Debug for ElementaryConstraint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0 {
            f.write_str("C(Always)")
        } else {
            f.write_str("C(Impossible)")
        }
    }
}

pub fn elementary(b: bool) -> Constraint {
    Constraint::Elementary(ElementaryConstraint(b))
}

pub fn unary(target: Operand, operator: UnaryOperator) -> Constraint {
    Constraint::Unary(UnaryConstraint { target, operator })
}

pub fn binary(target: Operand, operator: BinaryOperator, wanted: Operand) -> Constraint {
    Constraint::Binary(BinaryConstraint {
        target,
        operator,
        wanted_result: wanted,
    })
}

pub fn dedup_constraints(c: Vec<Constraint>, state: &mut State) -> Vec<Constraint> {
    let mut c = c;
    let mut dedup = Vec::new();

    let mut hs = HashSet::new();

    let mut i = 0;
    while i < c.len() {
        let con = c[c.len() - 1 - i];

        if !hs.insert(con) {
            c.remove(i);
            continue;
        }

        // impossible, whole chain is impossible
        if con.eq(&elementary(false)) {
            return vec![con];
        }

        dedup.append(&mut con.flatten(state));
        i += 1;
    }

    return dedup;
}

pub fn total_dedup_constraints(c: Vec<Constraint>, state: &mut State) -> Vec<Constraint> {
    let mut c = c;
    let mut p_hash = 0;
    let mut hash = compute_hash(&c);

    while p_hash != hash {
        p_hash = hash;
        c = dedup_constraints(c, state);
        hash = compute_hash(&c);
    }

    return c;
}

pub fn dedup_results(c: Vec<TokenResult>) -> Vec<TokenResult> {
    let mut c = c;
    let mut dedup = Vec::new();

    let mut hs = HashSet::new();

    let mut i = 0;
    while i < c.len() {
        let con = c[c.len() - 1 - i].clone();

        if !hs.insert(con.clone()) {
            c.remove(i);
            continue;
        }

        // TODO: this needs to properly merge results of instructions into results of the gadget..
        dedup.push(con);
        i += 1;
    }

    return dedup;
}

pub fn total_dedup_results(c: Vec<TokenResult>) -> Vec<TokenResult> {
    let mut c = c;
    let mut p_hash = 0;
    let mut hash = compute_hash(&c);

    while p_hash != hash {
        p_hash = hash;
        c = dedup_results(c);
        hash = compute_hash(&c);
    }

    return c;
}

pub fn is_constraint_vec_elementary(p: &Vec<Constraint>) -> Option<bool> {
    // Matter of definitions
    if p.len() == 0 {
        return Some(true);
    }

    if p.len() == 1
        && let Some(Constraint::Elementary(ElementaryConstraint(b))) = p.last()
    {
        return Some(*b);
    }

    None
}

pub fn is_subset<T: std::cmp::PartialEq>(a: &Vec<T>, b: &Vec<T>) -> bool {
    a.iter().all(|c| b.contains(c))
}

pub fn is_equal<T: std::cmp::PartialEq>(a: &Vec<T>, b: &Vec<T>) -> bool {
    is_subset(a, b) && is_subset(b, a)
}
