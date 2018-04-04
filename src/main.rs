use std::fs::File;
use std::io::prelude::*;

use wasm_read::DebugSections;
use reloc::reloc;
use dwarf::print_debug_loc;

extern crate gimli;
extern crate wasmparser;

mod wasm_read;
mod reloc;
mod dwarf;

fn main() {
    let perform_reloc = !true;
    let filename = "test/hi.2.wasm";
    let mut f = File::open(filename).expect("file not found");
    let mut data = Vec::new();
    f.read_to_end(&mut data).expect("unable to read file");

    let mut debug_sections = DebugSections::read_sections(data.as_slice());

    if perform_reloc {
      reloc(&mut debug_sections);
    }

    print_debug_loc(&debug_sections);
}
