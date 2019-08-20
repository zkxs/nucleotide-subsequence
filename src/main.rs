#[macro_use]
extern crate lazy_static;
extern crate slice_deque;

use std::{cmp, mem};
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, StdinLock};

use slice_deque::SliceDeque;

use config::Config;

mod config;

const VERBOSITY_INFO: u64 = 1;
const VERBOSITY_DEBUG: u64 = 2;

fn main() -> Result<(), String> {
    let config = match Config::from_args() {
        Ok(config) => config,
        Err(error) => return Err(error)
    };

    if config.target().len() == 0 {
        return Err("Target string cannot be empty".into());
    }

    // silly early variable declarations so they live until the end of main()
    //TODO: how to not do this shit?
    let mut buf_file: BufReader<File>;
    let stdin = io::stdin();
    let mut stdin_lock: StdinLock;

    let source: &mut dyn BufRead = match config.matches.value_of("file") {
        Some(filename) => {
            let file = match File::open(filename) {
                Ok(file) => file,
                Err(error) => {
                    return Err(error.to_string());
                }
            };
            buf_file = BufReader::new(file);
            &mut buf_file
        }
        None => {
            stdin_lock = stdin.lock();
            &mut stdin_lock
        }
    };

    match do_read(source, &config) {
        Ok(_) => {
            if config.verbosity >= VERBOSITY_INFO {
                println!("Done.");
            }
            Ok(())
        }
        Err(error) => Err(error.into())
    }
}


fn do_read(read: &mut dyn BufRead, config: &Config) -> Result<(), String> {
    let capacity = config.before_context + config.target().len() + config.after_context;

    // must be mutable for the unsafe implementation
    let mut initial_buffer: Vec<u8> = Vec::with_capacity(capacity);

    // unsafe implementation. Uses uninitialized memory in the buffer, because why not?
    let read_count: usize;
    unsafe {
        initial_buffer.set_len(capacity);
        read_count = match read.read(initial_buffer.as_mut_slice()) {
            Ok(read_count) => read_count,
            Err(error) => return Err(error.to_string())
        };
        initial_buffer.set_len(cmp::min(capacity, read_count));
    }

//    // alternate, safe implementation. Zero-fills the vector before using as a buffer.
//    let mut initial_buffer: Vec<u8> = Vec::with_capacity(capacity);
//    initial_buffer.resize(capacity, 0); // unnecessary zero-fill
//    let read_count = read.read(initial_buffer.as_mut_slice()).unwrap();
//    initial_buffer.truncate(read_count);

    if read_count < config.target().len() {
        return Err("stream too small to match on".into());
    } else {
        if config.verbosity >= VERBOSITY_DEBUG {
            // should be the same as capacity
            println!("read_count: {}", read_count);
        }

        // move initial buffer into the deque
        let mut buffer: SliceDeque<u8>;
        unsafe {
            let slice = initial_buffer.as_slice();
            buffer = SliceDeque::steal_from_slice(slice);
            mem::forget(slice);
        }

        if config.verbosity >= VERBOSITY_DEBUG {
            // show off all the wasted space
            println!("buffer_capacity: {}", buffer.capacity());
        }

        let mut before_offset: usize = 0;
        let mut target_offset: usize = 0;
        let mut after_offset: usize = config.target().len();
        let mut end_offset: usize = cmp::min(after_offset + config.after_context, read_count);
        let mut global_target_offset: u64 = 0;

        // handle missing before_context (e.g. match at start)
        // we gradually grow the before_context.
        // we always try the max after_context.
        loop {
            do_check(&buffer, config, before_offset, target_offset, after_offset, end_offset, &global_target_offset);

            if target_offset + 1 <= config.before_context && end_offset + 1 <= read_count {
                target_offset += 1;
                after_offset += 1;
                end_offset += 1;
            } else {
                break;
            }
        }

        if config.verbosity >= VERBOSITY_DEBUG {
            println!("done with prescan");
        }

        let bytes = read.bytes();

        // read the infinite stream beyond our initial load
        for maybe_byte in bytes {
            let byte = match maybe_byte {
                Ok(byte) => byte,
                Err(error) => return Err(error.to_string())
            };
            buffer.pop_front();
            buffer.push_back(byte);
            global_target_offset += 1;
            do_check(&buffer, config, before_offset, target_offset, after_offset, end_offset, &global_target_offset);
        }

        if config.verbosity >= VERBOSITY_DEBUG {
            println!("done with infinite scan");
        }

        // grow before_context for the edge case where we didn't have very much data
        if config.before_context > target_offset {
            before_offset = 0
        } else {
            before_offset = target_offset - config.before_context
        }

        // handle missing after_context (e.g. match at end)
        // we always try the max before_context
        // we gradually shrink after_context.
        loop {
            if target_offset >= config.before_context {
                before_offset += 1;
            } // else handle case where we didn't have enough input for a full-sized before_context

            target_offset += 1;
            after_offset += 1;
            if after_offset <= end_offset {
                do_check(&buffer, config, before_offset, target_offset, after_offset, end_offset, &global_target_offset);
            } else {
                break;
            }
        }
    }

    Ok(())
}

fn do_check(buffer: &SliceDeque<u8>, config: &Config, before_offset: usize, target_offset: usize, after_offset: usize, end_offset: usize, global_target_offset: &u64) {
    let slice = buffer.as_slice();
    let entire_string = String::from_utf8_lossy(slice);

    if config.verbosity >= VERBOSITY_DEBUG {
        println!("[u8, {}] -> [{}..{}..{}..{}]", slice.len(), before_offset, target_offset, after_offset, end_offset);
    }

    let match_slice = &slice[target_offset..after_offset];
    let matched = config.target() == match_slice;

    if matched {
        let match_offset = global_target_offset + target_offset as u64;
        let before = String::from_utf8_lossy(&slice[before_offset..target_offset]);
        let after = String::from_utf8_lossy(&slice[after_offset..end_offset]);
        if config.verbosity >= VERBOSITY_DEBUG {
            println!("{:?} @{} {:?} == {:?} -> true. b: {:?}, a: {:?}", entire_string, match_offset, String::from_utf8_lossy(config.target()), String::from_utf8_lossy(match_slice), before, after);
        } else {
            if config.pad {
                print!("{:4}: ", match_offset);

                // pad the before context string so that all the matches line up nicely
                for _ in 0..config.before_context - before.len() {
                    print!(" ");
                }

                println!("{}[MATCH]{}", before, after);
            } else {
                println!("{}: {}[MATCH]{}", match_offset, before, after);
            }
        }
    } else {
        let match_offset = global_target_offset + target_offset as u64;
        if config.verbosity >= VERBOSITY_DEBUG {
            println!("{:?} @{} {:?} == {:?} -> false", entire_string, match_offset, String::from_utf8_lossy(config.target()), String::from_utf8_lossy(match_slice));
        }
    }
}
