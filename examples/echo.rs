#![feature(generators)]

#[macro_use] extern crate emp;
use emp::prelude::*;

#[derive(Debug)]
struct Foo;

#[derive(Debug)]
struct Bar(u32);

#[derive(Debug)]
struct Baz(String);

fn main() {
    let mut d = Dispatcher::new();
    let printer = d.spawn(dispatcher! {
        String, s => { 
            println!("printer: {}", s);
        }
    });
    let handler = d.spawn(dispatcher! {
        Connection, c => { 
            let remote = c.remote();
            println!("connection from: {:?}", remote);
            let handler = spawn!(dispatcher! {
                Line, Line(s) => {
                    yield_to!(printer, format!("recieved {} from {}", s, remote));
                },
                Closed, _ => break
            });
            spawn!(line_reader(c, handler));
        }
    });
    
    d.spawn(listener([127, 0, 0, 1].into(), 1337, handler));
    d.run();
}
