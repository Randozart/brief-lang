// Rust host for the Briev GLUE library — plain C ABI calls, no marshalling.
mod briev_bindings;

use briev_bindings::*;
use std::ffi::c_void;

fn main() {
    unsafe {
        let state: *mut c_void = __briev_init_state();
        let h = feature_hash(state, 1000, 42);
        let a = add(3, 4);
        println!("feature_hash(1000, 42) = {h}");
        println!("add(3, 4) = {a}");
        __glue_release(state);
    }
}
