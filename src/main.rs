#![windows_subsystem = "windows"]

mod capture;
mod classical_worker;
mod core;
mod gui;
mod input;
mod matching;
#[cfg(test)]
mod tests;
mod worker;

fn main() {
    gui::run();
}
