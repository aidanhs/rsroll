rollsum is based on bupsplit, which in turn is based on rsync chunking.

Interface liable to change.

https://docs.rs/rollsum

```
extern crate rollsum;

use std::env;
use std::fs;
use std::path::Path;
use std::io::prelude::*;

pub fn main () {
    let args: Vec<_> = env::args().collect();
    let mut file = fs::File::open(&Path::new(&args[1])).unwrap();
    let mut buf = vec![];
    file.read_to_end(&mut buf).unwrap();

    let mut ofs: usize = 0;
    while ofs < buf.len() {
        let mut b = rollsum::Bup::new();
        if let Some(count) = b.find_chunk_edge(&buf[ofs..]) {
            ofs += count;
            println!("found edge at {}", ofs);
        } else {
            println!("end of the line!");
            break
        }
    }
}
```
