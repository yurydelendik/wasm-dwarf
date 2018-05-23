use std::env;
use std::fs::File;
use std::io::prelude::*;

use dwarf::get_debug_loc;
use getopts::Options;
use reloc::reloc;
use to_json::convert_debug_info_to_json;
use wasm_read::{add_source_mapping_url_section, remove_debug_sections, DebugSections};

extern crate getopts;
extern crate gimli;
extern crate rustc_serialize;
extern crate vlq;
extern crate wasmparser;

mod dwarf;
mod reloc;
mod to_json;
mod wasm_read;

struct PrefixReplacements {
    replacements: Vec<(String, String)>,
}

impl PrefixReplacements {
    fn parse(input: &Vec<String>) -> PrefixReplacements {
        let mut replacements = Vec::new();
        for i in input.iter() {
            let separator = i.find('=');
            if let Some(separator_index) = separator {
                replacements.push((
                    i.chars().take(separator_index).collect(),
                    i.chars()
                        .skip(separator_index + 1)
                        .take(i.len() - 1 - separator_index)
                        .collect(),
                ));
            } else {
                replacements.push((i.clone(), String::new()))
            }
        }
        return PrefixReplacements { replacements };
    }

    fn replace(&self, path: &String) -> String {
        let mut result = path.clone();
        for (ref old_prefix, ref new_prefix) in self.replacements.iter() {
            if path.starts_with(old_prefix) {
                result = result.split_off(old_prefix.len());
                result.insert_str(0, new_prefix);
                return result;
            }
        }
        result
    }

    fn replace_all(&self, paths: &mut Vec<String>) {
        for path in paths.iter_mut() {
            *path = self.replace(&path);
        }
    }
}

fn main() {
    let mut opts = Options::new();
    opts.optopt("o", "", "set output file name", "NAME");
    opts.optflag("", "relocation", "perform relocation first");
    opts.optflag("d", "dump", "print source files and location entries");
    opts.optmulti(
        "p",
        "prefix",
        "replace source filename prefix",
        "OLD_PREFIX[=NEW_PREFIX]",
    );
    opts.optflag("s", "sources", "read and embed source files");
    opts.optopt("w", "", "set output wasm file", "NAME");
    opts.optflag("x", "strip", "removes debug and linking sections");
    opts.optopt(
        "m",
        "source-map",
        "specifies sourceMappingURL section contest",
        "URL",
    );
    opts.optflag("h", "help", "print this help menu");

    let args: Vec<_> = env::args().collect();
    let program = args[0].clone();
    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => panic!(f.to_string()),
    };
    if matches.opt_present("h") || matches.free.len() < 1
        || !(matches.opt_present("o") || matches.opt_present("d"))
    {
        return print_usage(&program, opts);
    }

    let perform_reloc = matches.opt_present("relocation");
    let filename = matches.free[0].clone();
    let mut f = File::open(filename).expect("file not found");
    let mut data = Vec::new();
    f.read_to_end(&mut data).expect("unable to read file");

    let mut debug_sections = DebugSections::read_sections(data.as_slice());

    if perform_reloc {
        if debug_sections.linking.is_none() {
            panic!("relocation information was not found");
        }
        reloc(&mut debug_sections);
    }

    let as_json = matches.opt_present("o");
    let mut di = get_debug_loc(&debug_sections);

    if matches.opt_present("sources") {
        let mut sources = Vec::new();
        for file in di.sources.iter() {
            let mut f = File::open(file).expect("file not found");
            let mut data = Vec::new();
            f.read_to_end(&mut data).expect("unable to read file");
            sources.push(String::from_utf8(data).unwrap());
        }
        di.sources_content = Some(sources);
    }

    if matches.opt_present("prefix") {
        let prefix_replacements = PrefixReplacements::parse(&matches.opt_strs("prefix"));
        prefix_replacements.replace_all(&mut di.sources);
    }

    if as_json {
        let output = matches.opt_str("o").unwrap();
        let result = convert_debug_info_to_json(&di).to_string();
        if output == "-" {
            println!("{}", result);
        } else {
            let mut f_out = File::create(output).expect("file cannot be created");
            f_out.write(result.as_bytes()).expect("data written");
        }
    } else {
        for (id, path) in di.sources.iter().enumerate() {
            println!("source {}: {}", id, path);
        }
        for loc in di.locations {
            println!(
                "{:x} @ {},{} ({})",
                loc.address, loc.line, loc.column, loc.source_id
            );
        }
    }

    if matches.opt_present("w") {
        let wasm_output = matches.opt_str("w").unwrap();
        let mut modified_wasm = None;
        if matches.opt_present("x") {
            modified_wasm = Some(Vec::new());
            remove_debug_sections(&data, modified_wasm.as_mut().unwrap());
        }
        if matches.opt_present("m") {
            if modified_wasm.is_none() {
                modified_wasm = Some(Vec::new());
                modified_wasm.as_mut().unwrap().extend_from_slice(&data);
            }
            let url = matches.opt_str("m").unwrap();
            add_source_mapping_url_section(&url, modified_wasm.as_mut().unwrap());
        }
        let mut f_out = File::create(wasm_output).expect("file cannot be created");
        f_out
            .write(&modified_wasm.unwrap_or(data))
            .expect("wasm data written");
    }
}

fn print_usage(program: &str, opts: Options) {
    let brief = format!("Usage: {} [options] <INPUT>", program);
    print!("{}", opts.usage(&brief));
    println!(
        "
Reading DWARF data from the wasm object files, and converting to source maps.

Usage:

    # Read and convert to JSON
    wasm-dwarf foo.wasm -o foo.map
"
    );
}
