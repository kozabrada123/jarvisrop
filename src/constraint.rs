use std::{
    collections::{HashSet, VecDeque},
    fmt,
};

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
            iced_x86::Mnemonic::Mov | iced_x86::Mnemonic::Cmp | iced_x86::Mnemonic::Test => {
                let dst = self.operands[0].unwrap();
                let src = self.operands[1].unwrap();

                let mut c = Vec::with_capacity(3);

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
                        c.push(unary(dst, UnaryOperator::IsReadableAddr));
                    }
                    _ => {}
                }

                return c;
            }
            iced_x86::Mnemonic::Pop => {
                vec![elementary(true)]
            }
            iced_x86::Mnemonic::Ret => {
                // Don't bother with ret 0x4a95; and such
                if self.operands[0].is_some() {
                    vec![elementary(false)];
                }
                vec![elementary(true)]
            }
            iced_x86::Mnemonic::Nop => {
                vec![elementary(true)]
            }
            iced_x86::Mnemonic::Clc | iced_x86::Mnemonic::Cli | iced_x86::Mnemonic::Cld => {
                vec![elementary(true)]
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

                let requirements = vec![elementary(true)];
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
            iced_x86::Mnemonic::Ret => {
                vec![]
            }
            iced_x86::Mnemonic::Clc | iced_x86::Mnemonic::Cli | iced_x86::Mnemonic::Cld => {
                vec![TokenResult {
                    requirements: vec![elementary(true)],
                    effects: vec![],
                }]
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

impl TokenResult {
    /// How would self, if executed after o, overwrite it?
    pub fn overwrite(&self, o: &mut TokenResult) {
        let mut i = 0;
        while i < o.effects.len() {
            let eff = o.effects[i].clone();

            for later_eff in &self.effects {
                if later_eff.overwrites(&eff) {
                    o.effects.remove(i);
                    i -= 1;
                    break;
                }
            }

            i += 1;
        }
    }
}

impl fmt::Debug for TokenResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!(
            "R({:?} -> {:?})",
            self.requirements, self.effects
        ))
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
    pub fn can_be_transposed_as(
        self,
        other: &TokenResult,
        new_requirements: Option<&mut Vec<Constraint>>,
    ) -> bool {
        // Word for word the same
        if other.effects.contains(&self) {
            return true;
        }

        match self {
            Self::Unary(s) => match s.operator {
                // Is controlled can be written as equals something else that is controlled
                UnaryOperator::IsControlled => {
                    for effect in &other.effects {
                        match effect {
                            Self::Binary(o) => match o.operator {
                                BinaryOperator::IsEqual => {
                                    // FIXME: op_a_part_of_b can cause bugs
                                    // TODO: memory goofyness
                                    if o.wanted_result.is_register()
                                        && op_a_part_of_b(s.target, o.target)
                                    {
                                        if let Some(vec) = new_requirements {
                                            vec.push(unary(
                                                o.wanted_result,
                                                UnaryOperator::IsControlled,
                                            ));
                                        }
                                        return true;
                                    }
                                }
                                _ => {}
                            },
                            Self::Unary(o) => {
                                // Same thing we're doing, just with a larger sized register
                                if s.operator == o.operator {
                                    return op_a_part_of_b(s.target, o.target);
                                }
                            }
                            _ => {}
                        }
                    }

                    false
                }
                _ => false,
            },
            Self::Binary(s) => match s.operator {
                // TODO: rewrite is not equal to is equal / is lower / is greater than something
                // else
                BinaryOperator::IsEqual => {
                    for effect in &other.effects {
                        match effect {
                            Self::Binary(o) => match o.operator {
                                BinaryOperator::IsEqual => {
                                    // FIXME: can cause bugs
                                    if o.wanted_result.is_register()
                                        && op_a_part_of_b(s.target, o.target)
                                    {
                                        if let Some(vec) = new_requirements {
                                            vec.push(binary(
                                                o.wanted_result,
                                                BinaryOperator::IsEqual,
                                                s.wanted_result,
                                            ));
                                        }
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

    /// Would self, if executed after o, overwrite it?
    pub fn overwrites(&self, o: &Constraint) -> bool {
        match self {
            Constraint::Elementary(_) => false,
            Constraint::Unary(u) => u.overwrites(o),
            Constraint::Binary(b) => b.overwrites(o),
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
        f.write_fmt(format_args!(
            "C({:?} {:?} {:?})",
            self.target, self.operator, self.wanted_result
        ))
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

    /// Would self, if executed after o, overwrite it?
    pub fn overwrites(&self, o: &Constraint) -> bool {
        match o {
            Constraint::Binary(b) => {
                if !op_overlaps(b.target, self.target) {
                    return false;
                }

                // TODO: direct evaluation with immediates, ...
                if self.operator == BinaryOperator::IsEqual && b.operator == BinaryOperator::IsEqual
                {
                    return true;
                }

                // TODO: can't know for sure
                return false;
            }
            Constraint::Unary(u) => {
                if !op_overlaps(u.target, self.target) {
                    return false;
                }

                if self.operator == BinaryOperator::IsEqual
                    && u.operator == UnaryOperator::IsControlled
                {
                    return true;
                }

                // TODO: can't know for sure
                return false;
            }
            Constraint::Elementary(_) => false,
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

    IsUnchanged,
    IsControlled,
}

impl UnaryConstraint {
    // This should probably be a part of "can be rewritten as"
    /// Convert self into a list of constraints self depends on
    ///
    /// self.flatten => self
    pub fn flatten(self, state: &mut State) -> Vec<Constraint> {
        match self.target {
            Operand::Memory(_memory) => {
                match self.operator {
                    UnaryOperator::IsControlled | UnaryOperator::IsUnchanged => {
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

                    UnaryOperator::IsUnchanged => {
                        if reg == Register::BP || reg == Register::RBP {
                            return vec![elementary(false)];
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

                    // Dumb, will always be true
                    UnaryOperator::IsUnchanged => {
                        vec![elementary(true)]
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

    /// Would self, if executed after o, overwrite it?
    ///
    /// Or, would self o, if needed before self, invalidate self?
    pub fn overwrites(&self, o: &Constraint) -> bool {
        match o {
            Constraint::Binary(b) => {
                if !op_overlaps(b.target, self.target) {
                    return false;
                }

                if b.operator == BinaryOperator::IsEqual {
                    return true;
                }

                // TODO: can't know for sure
                return false;
            }
            Constraint::Unary(u) => {
                if !op_overlaps(u.target, self.target) {
                    return false;
                }

                if self.operator == UnaryOperator::IsControlled {
                    return true;
                }

                // TODO: can't know for sure
                return false;
            }
            Constraint::Elementary(_) => false,
        }
    }
}

/// ElementaryConstraint(true) is always true, such as 1 = 1
/// ElementaryConstraint(false) is always false, such as 1 = 0
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ElementaryConstraint(pub bool);

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

/// Assume that bigger indexes are constraints that are needed earlier
///
/// as_effects dictates if we treat these as wanted constraints or as effects;
/// if set to true, later ones can overwrite earlier ones
pub fn dedup_constraints(c: &mut Vec<Constraint>, as_effects: bool) {
    let mut hs = HashSet::new();

    let mut i = 0;
    while i < c.len() {
        let con = c[i];

        if con == elementary(true) {
            c.remove(i);
            continue;
        }

        if !hs.insert(con) {
            c.remove(i);
            continue;
        }

        // impossible, whole chain is impossible
        if con.eq(&elementary(false)) {
            c.clear();
            c.push(con);
            return;
        }

        // Check for later (earlier) effects that get overwritten by this one
        // or
        // Check for later (earlier) constraints that we conflict with
        let mut j = i + 1;
        while j < c.len() {
            let later_con = &c[j];

            if as_effects {
                if con.overwrites(later_con) {
                    c.remove(j);
                    continue;
                }
            }
            else {
                // Later constraint is in conflict with us, it was never meant to be
                if &con != later_con && later_con.overwrites(&con) {
                    c[i] = elementary(false);
                    return;
                }
            }

            j += 1;
        }

        i += 1;
    }
}

pub fn total_dedup_constraints(c: &mut Vec<Constraint>, as_effects: bool) {
    let mut c = c;
    let mut c_len = c.len();
    let mut p_len = 0;

    while c_len != p_len {
        p_len = c.len();
        dedup_constraints(&mut c, as_effects);
        c_len = c.len();
    }

    // move from "just control -1 bro" to "yeah that's impossible dipshit"
    dedup_constraints(&mut c, as_effects);
}

/// Assumes that larger indexes are results that happened *earlier*
pub fn dedup_results(r: &mut Vec<TokenResult>) {
    let mut hs = HashSet::new();

    let mut i = 0;
    while i < r.len() {
        let res = r[i].clone();

        if !hs.insert(res.clone()) {
            r.remove(i);
            continue;
        }

        // TODO: should be improved: check for mov rax, rcx; mov rcx; rax
        let res_plus_1_o = r.get(i + 1);

        if let Some(res_plus_1) = res_plus_1_o
            && res.effects.len() == 1
            && res_plus_1.effects.len() == 1
        {
            let e1 = res.effects[0];
            let e2 = res_plus_1.effects[0];

            match (e1, e2) {
                (Constraint::Binary(b1), Constraint::Binary(b2)) => {
                    let movs = b1.operator == BinaryOperator::IsEqual && b1.operator == b2.operator;
                    let opposing = b1.target == b2.wanted_result && b1.wanted_result == b2.target;

                    if movs && opposing {
                        // mov 2, 1
                        // mov 1, 2
                        // -> the second is a no-op
                        r.remove(i);
                        continue;
                    }
                }
                _ => {}
            }
        }

        // check for later (earlier) effects that get overwriten by this one
        for j in (i + 1)..r.len() {
            let later_con = &mut r[j];

            res.overwrite(later_con);
        }

        // TODO: this needs to properly merge results of instructions into results of the gadget..
        i += 1;
    }
}

pub fn total_dedup_results(c: &mut Vec<TokenResult>) {
    let mut c_len = c.len();
    let mut p_len = 0;

    while c_len != p_len {
        p_len = c.len();
        dedup_results(c);
        c_len = c.len();
    }

    // move from "just control -1 bro" to "yeah that's impossible dipshit"
    dedup_results(c);
}

pub fn is_constraint_vec_elementary(p: &Vec<Constraint>) -> Option<bool> {
    // Matter of definitions
    if p.len() == 0 {
        return Some(true);
    }

    if p.len() == 1
        && let Constraint::Elementary(ElementaryConstraint(b)) = p[0]
    {
        return Some(b);
    }

    None
}

pub fn is_a_subset_of_b<T: std::cmp::PartialEq>(a: &Vec<T>, b: &Vec<T>) -> bool {
    a.iter().all(|c| b.contains(c))
}

pub fn is_equal<T: std::cmp::PartialEq>(a: &Vec<T>, b: &Vec<T>) -> bool {
    is_a_subset_of_b(a, b) && is_a_subset_of_b(b, a)
}

pub fn op_overlaps(a: Operand, b: Operand) -> bool {
    // TODO: memory n shi
    if let Operand::Register(ar) = a
        && let Operand::Register(br) = b
    {
        return overlaps(ar, br);
    }

    false
}

pub fn overlaps(a: iced_x86::Register, b: iced_x86::Register) -> bool {
    if a == b {
        return true;
    }

    let full_a = a.full_register();
    let full_b = b.full_register();

    let full_a_not_none = full_a != iced_x86::Register::None;
    let full_b_not_none = full_b != iced_x86::Register::None;

    if full_a_not_none && full_b_not_none {
        return full_a == full_b;
    }

    if full_a_not_none {
        return full_a == b;
    } else if full_a_not_none {
        return a == full_b;
    }

    return false;
}

pub fn op_a_part_of_b(a: Operand, b: Operand) -> bool {
    // TODO: memory n shi
    if let Operand::Register(ar) = a
        && let Operand::Register(br) = b
    {
        return a_part_of_b(ar, br);
    }

    false
}

/// Returns true if a is smaller or equal to b
pub fn a_part_of_b(a: iced_x86::Register, b: iced_x86::Register) -> bool {
    if a == b {
        return true;
    }

    let full_a = a.full_register();
    let full_b = b.full_register();

    let full_a_not_none = full_a != iced_x86::Register::None;
    let full_b_not_none = full_b != iced_x86::Register::None;

    if full_a_not_none && full_a == b {
        return true;
    }

    if full_a_not_none && full_b_not_none && (full_a == full_b) {
        return a.size() <= b.size();
    }

    return false;
}
