// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception

witgenstein_rs_macros::component! {
    #[export]
    pub fn add(a: u32, b: u32) -> u32 {
        a + b
    }

    #[export]
    pub fn greet(name: String) -> String {
        format!("Hello, {name}!")
    }
}
