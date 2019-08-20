extern crate clap;
extern crate regex;

use std::fs::File;
use std::io::Read;

use clap::{App, Arg, ArgMatches};
use regex::Regex;

pub struct Config<'a> {
    pub matches: ArgMatches<'a>,
    pub verbosity: u64,
    raw_target: Vec<u8>,
    pub before_context: usize,
    pub after_context: usize,
    pub pad: bool,
}

impl<'a> Config<'a> {
    pub fn from_args() -> Result<Config<'a>, String> {
        let matches = App::new("nucleotide-subsequence")
            .version("0.1.0")
            .author("Michael Ripley <zkxs00@gmail.com")
            .about("Finds target subsequences with context from some infinite stream")
            .arg(Arg::with_name("verbosity")
                .long("verbose")
                .short("v")
                .multiple(true)
                .help("Sets the level of verbosity."))
            .arg(Arg::with_name("file")
                .long("file")
                .short("f")
                .help("Specifies a file to read from, instead of stdin.")
                .value_name("FILE")
                .takes_value(true))
            .arg(Arg::with_name("target_string")
                .short("t")
                .long("target")
                .help("Sets the target string to search for.")
                .value_name("TARGET")
                .takes_value(true)
                .conflicts_with("target_file")
                .required(true))
            .arg(Arg::with_name("target_file")
                .short("z")
                .long("target-file")
                .help("Sets a file to read the target string from.")
                .value_name("FILE")
                .takes_value(true)
                .conflicts_with("target_string")
                .required(true))
            .arg(Arg::with_name("context")
                .short("c")
                .long("context")
                .help("Show BYTES bytes of context on either side of the match.")
                .value_name("BYTES")
                .takes_value(true)
                .conflicts_with_all(&["before_context", "after_context"])
                .validator(is_number))
            .arg(Arg::with_name("before_context")
                .short("b")
                .long("before")
                .help("Show BYTES bytes of context before the match.")
                .value_name("BYTES")
                .takes_value(true)
                .conflicts_with("context")
                .validator(is_number))
            .arg(Arg::with_name("after_context")
                .short("a")
                .long("after")
                .help("Show BYTES bytes of context after the match.")
                .value_name("BYTES")
                .takes_value(true)
                .conflicts_with("context")
                .validator(is_number))
            .arg(Arg::with_name("no_pad")
                .short("p")
                .long("no-pad")
                .help("Disables output padding. Output will not be aligned, but will be easier to parse."))
            .get_matches();

        Config::build(matches)
    }

    #[inline]
    fn build(matches: ArgMatches) -> Result<Config, String> {
        let verbosity = matches.occurrences_of("verbosity");

        let raw_target: Vec<u8> = if matches.is_present("target_string") {
            matches.value_of("target_string").unwrap().into() // wtf how the hell did into() just work!?
        } else {
            let filename = matches.value_of("target_file").unwrap();
            let mut buffer: Vec<u8> = Vec::new();
            let mut file = File::open(filename).unwrap();
            let result = file.read_to_end(&mut buffer);
            match result {
                Ok(_) => {}
                Err(error) => return Err(error.to_string())
            }
            buffer
        };

        let before_context: usize = matches.value_of("context")
            .map(|n| n.parse().unwrap())
            .unwrap_or_else(|| matches.value_of("before_context")
                .map(|n| n.parse().unwrap())
                .unwrap_or(0 as usize)
            );

        let after_context: usize = matches.value_of("context")
            .map(|n| n.parse().unwrap())
            .unwrap_or_else(|| matches.value_of("after_context")
                .map(|n| n.parse().unwrap())
                .unwrap_or(0 as usize)
            );

        let pad = !matches.is_present("no_pad");

        Ok(
            Config {
                matches,
                verbosity,
                raw_target,
                before_context,
                after_context,
                pad,
            }
        )
    }

    #[inline]
    pub fn target(&self) -> &[u8] {
        self.raw_target.as_slice()
    }
}

fn is_number(v: String) -> Result<(), String> {
    lazy_static! {
        static ref NUMBER: Regex = Regex::new(r#"^[1-9][0-9]*$"#).unwrap();
    }
    if NUMBER.is_match(&v) {
        Ok(())
    } else {
        Err(String::from("The value is not numeric"))
    }
}
