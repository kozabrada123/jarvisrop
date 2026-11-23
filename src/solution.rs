use std::{
    sync::{Arc, Mutex},
    thread::JoinHandle,
};

use crate::search::Path;

#[derive(Debug)]
pub struct Solution {
    pub chains: Vec<Path>,
    pub timer: Option<JoinHandle<()>>,
}

impl Solution {
    fn timer_thread(s: Arc<Mutex<Solution>>) {
        std::thread::sleep(std::time::Duration::from_millis(1000));
        s.lock()
            .expect("Failed to get solve lock!")
            .print_and_exit();
    }

    /// Adds a chain to our solution, triggering the ending timer if we haven't already
    pub fn add(s: &Arc<Mutex<Solution>>, path: Path) {
        let mut lock = s.lock().expect("Failed to get solve lock!");
        lock.chains.push(path);

        if lock.timer.is_none() {
            let s_clone = s.clone();
            lock.timer = Some(std::thread::spawn(move || {
                Self::timer_thread(s_clone);
            }));
        }
    }

    /// Sort the solutions to show the most ideal ones first
    pub fn sort(&mut self) {
        self.chains.sort_by(|x, y| {
            x.ending_state
                .fullfilled_effects
                .len()
                .cmp(&y.ending_state.fullfilled_effects.len())
                .then(x.executed_gadgets.len().cmp(&y.executed_gadgets.len()))
        });
    }

    pub fn print_chain(chain: &Path) {
        let mut solve_str = String::with_capacity(128);

        solve_str.push_str("End state:\n");

        if !chain.ending_state.wanted_constraints.is_empty() {
            solve_str.push_str("- constraints:\n");
            for constraint in &chain.ending_state.wanted_constraints {
                solve_str.push_str(&format!("  - {:?}\n", constraint));
            }
            solve_str.push_str("\n");
        }

        solve_str.push_str("- fullfilled effects:\n");
        for effect in chain.ending_state.fullfilled_effects.iter().rev() {
            solve_str.push_str(&format!("  - {:?}\n", effect));
        }
        solve_str.push_str("\nGadgets:\n");
        for gadget in chain.executed_gadgets.iter().rev() {
            solve_str.push_str(&format!("- {:?}\n", gadget));
        }
        solve_str.push_str("\n");

        print!("{}", solve_str);
    }

    pub fn print_and_exit(&self) {
        println!("Found {} chains:\n", self.chains.len());

        for chain in &self.chains {
            Self::print_chain(&chain);
        }

        std::process::exit(0);
    }
}
