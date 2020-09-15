#![feature(generators)]

#[macro_use] extern crate emp;
use emp::prelude::*;

#[derive(Debug)]
struct Foo(usize);

#[derive(Debug)]
struct Start;

#[derive(Debug)]
struct End;

use std::time::Instant;

fn main() {
    let mut d = Dispatcher::new();
    let mut foo_count = 0;
    let mut start = Instant::now();

    let test1 = d.spawn(dispatcher!{
        Foo, Foo(i) => {
            foo_count += 1;
        },
        Start, _ => {
            start = Instant::now();
        },
        End, _ => {
            let dt = start.elapsed();
            println!("got {} Foos in {:.3}s, ({:.1}ns each)", foo_count, dt.as_secs_f64(), 1e9 * dt.as_secs_f64() / foo_count as f64);
            exit!("done");
        }
    });
    
    let test2 = d.spawn(dispatcher!{
        Start, _ => {
            send!(test1, Start);
            for i in 0 .. 50_000_000 {
                send!(test1, Foo(i));
            }
            send!(test1, End);
        }
    });

    d.send(test2, Envelope::pack(Start));
    d.run();
}
