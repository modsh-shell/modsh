#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Fuzz the lexer with arbitrary byte input
    // Convert bytes to string, handling invalid UTF-8 by replacing with replacement character
    let input = String::from_utf8_lossy(data);

    // Attempt to tokenize - should never panic, only return Err
    let _ = modsh_core::lexer::tokenize(&input);
});
