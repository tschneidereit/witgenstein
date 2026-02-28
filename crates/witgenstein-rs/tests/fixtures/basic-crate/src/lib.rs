// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception

use witgenstein_rs_macros::export;

/// A simple point in 2D space.
pub struct Point {
    pub x: f64,
    pub y: f64,
}

/// A color enum.
pub enum Color {
    Red,
    Green,
    Blue,
}

/// A shape variant.
pub enum Shape {
    Circle(f64),
    Rectangle(f64, f64),
}

/// Add two numbers.
#[export]
pub fn add(a: u32, b: u32) -> u32 {
    a + b
}

/// Greet a person by name.
#[export]
pub fn greet(name: String) -> String {
    format!("Hello, {name}!")
}

/// Process a list of values.
#[export]
pub fn sum_list(values: Vec<f64>) -> f64 {
    values.iter().sum()
}

/// Try to parse a string as a number.
#[export]
pub fn try_parse(input: String) -> Result<u32, String> {
    input
        .parse()
        .map_err(|e: std::num::ParseIntError| e.to_string())
}

/// Get an optional greeting.
#[export]
pub fn maybe_greet(name: Option<String>) -> Option<String> {
    name.map(|n| format!("Hello, {n}!"))
}

/// Resource type with methods.
pub struct Counter {
    value: u32,
}

#[export]
impl Counter {
    pub fn new(initial: u32) -> Self {
        Self { value: initial }
    }

    pub fn get(&self) -> u32 {
        self.value
    }

    pub fn increment(&mut self) {
        self.value += 1;
    }

    pub fn add(&mut self, n: u32) {
        self.value += n;
    }
}
