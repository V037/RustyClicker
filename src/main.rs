#![windows_subsystem = "windows"]

mod capture;
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
