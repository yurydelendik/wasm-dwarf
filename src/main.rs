use std::fs::File;
use std::io::prelude::*;

use wasm_read::DebugSections;
use reloc::reloc;
use dwarf::get_debug_loc;
use to_json::convert_debug_info_to_json;

extern crate gimli;
extern crate wasmparser;
extern crate rustc_serialize;
extern crate vlq;

mod wasm_read;
mod reloc;
mod dwarf;
mod to_json;

fn main() {
    let perform_reloc = !true;
    let filename = "test/hi.3.wasm";
    let mut f = File::open(filename).expect("file not found");
    let mut data = Vec::new();
    f.read_to_end(&mut data).expect("unable to read file");

    let mut debug_sections = DebugSections::read_sections(data.as_slice());

    if perform_reloc {
      reloc(&mut debug_sections);
    }

    let as_json = !true;
    let di = get_debug_loc(&debug_sections);
    if as_json {
      println!("{}", convert_debug_info_to_json(&di).to_string());
    } else {
      for (id, path) in di.sources.iter().enumerate() {
        println!("source {}: {}", id, path);
      }
      for loc in di.locations {
        println!("{:x} @ {},{} ({})", loc.address, loc.line, loc.column, loc.source_id);
      }
    }
}
