use std::env;
use std::fs::File;
use std::io::prelude::*;

use wasm_read::DebugSections;
use reloc::reloc;
use dwarf::get_debug_loc;
use to_json::convert_debug_info_to_json;
use getopts::Options;

extern crate gimli;
extern crate wasmparser;
extern crate rustc_serialize;
extern crate vlq;
extern crate getopts;

mod wasm_read;
mod reloc;
mod dwarf;
mod to_json;

fn main() {
    let mut opts = Options::new();
    opts.optopt("o", "", "set output file name", "NAME");
    opts.optflag("", "relocation", "perform relocation first");
    opts.optflag("d", "dump", "print source files and location entries");
    opts.optflag("h", "help", "print this help menu");

    let args: Vec<_> = env::args().collect();
    let program = args[0].clone();
    let matches = match opts.parse(&args[1..]) {
      Ok(m) => { m }
      Err(f) => { panic!(f.to_string()) }
    };
    if matches.opt_present("h") || matches.free.len() < 1 ||
       !(matches.opt_present("o") || matches.opt_present("d")) {
      return print_usage(&program, opts);
    }

    let perform_reloc = matches.opt_present("relocation");
    let filename = matches.free[0].clone();
    let mut f = File::open(filename).expect("file not found");
    let mut data = Vec::new();
    f.read_to_end(&mut data).expect("unable to read file");

    let mut debug_sections = DebugSections::read_sections(data.as_slice());

    if perform_reloc {
      reloc(&mut debug_sections);
    }

    let as_json = matches.opt_present("o");
    let di = get_debug_loc(&debug_sections);
    if as_json {
      let output = matches.opt_str("o").unwrap();
      let result = convert_debug_info_to_json(&di).to_string();
      if output == "-" {
        println!("{}", result);
      } else {
        let mut f_out = File::create(output).expect("file can be created");
        f_out.write(result.as_bytes()).expect("data written");
      }
    } else {
      for (id, path) in di.sources.iter().enumerate() {
        println!("source {}: {}", id, path);
      }
      for loc in di.locations {
        println!("{:x} @ {},{} ({})", loc.address, loc.line, loc.column, loc.source_id);
      }
    }
}

fn print_usage(program: &str, opts: Options) {
    let brief = format!("Usage: {} [options] <INPUT>", program);
    print!("{}", opts.usage(&brief));
    println!("
Reading DWARF data from the wasm object files, and converting to source maps.

Usage:

    # Read and convert to JSON
    wasm-dwarf foo.wasm -o foo.map
");
}