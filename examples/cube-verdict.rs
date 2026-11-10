//! Prints whether the chart-profile reader (`cube::from_jsonld`) accepts each file.
//!
//! ```text
//! cargo run -q --example cube-verdict --features charts -- <file.jsonld>...
//! accept<TAB>path
//! reject<TAB>path<TAB>message
//! ```
//!
//! Exit status 0 when every file was read (accepted or rejected), 2 when a file
//! could not be read or no file was given. `tests/differential.sh` compares these
//! verdicts with pySHACL's on the vocabulary's fixtures.

use std::{env, fs, process::ExitCode};

use eona_ui_toolkit::cube::from_jsonld;

fn main() -> ExitCode {
    let files: Vec<String> = env::args().skip(1).collect();
    if files.is_empty() {
        eprintln!("usage: cube-verdict <file.jsonld>...");
        return ExitCode::from(2);
    }
    let mut status = ExitCode::SUCCESS;
    for path in files {
        match fs::read_to_string(&path) {
            Err(e) => {
                eprintln!("{path}: {e}");
                status = ExitCode::from(2);
            }
            Ok(text) => match from_jsonld(&text) {
                Ok(_) => println!("accept\t{path}"),
                Err(e) => println!("reject\t{path}\t{}", e.to_string().replace(['\n', '\t'], " ")),
            },
        }
    }
    status
}
