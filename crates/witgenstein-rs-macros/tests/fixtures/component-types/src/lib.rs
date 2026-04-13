// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception

// --- Records (discovered automatically from function signatures) ---
pub struct Point {
    pub x: f64,
    pub y: f64,
}

// --- Enums (discovered automatically from function signatures) ---
pub enum Color {
    Red,
    Green,
    Blue,
}

// --- Resource struct (outside the macro, impl inside) ---
pub struct Counter {
    value: u32,
}

witgenstein_rs_macros::component! {
    // --- Functions using named types ---
    #[export]
    pub fn color_name(color: Color) -> String {
        match color {
            Color::Red => "red".into(),
            Color::Green => "green".into(),
            Color::Blue => "blue".into(),
        }
    }

    #[export]
    pub fn distance(a: Point, b: Point) -> f64 {
        let dx = a.x - b.x;
        let dy = a.y - b.y;
        (dx * dx + dy * dy).sqrt()
    }

    #[export]
    pub fn try_parse(input: String) -> Result<u32, String> {
        input
            .parse()
            .map_err(|e: std::num::ParseIntError| e.to_string())
    }

    #[export]
    pub fn maybe_double(value: Option<u32>) -> Option<u32> {
        value.map(|v| v * 2)
    }

    // --- Resource impl ---
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
}
