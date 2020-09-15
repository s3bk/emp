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
    let handler = d.spawn(
        dispatcher!{
            Connection, c => {
                let (remote, port) = c.remote();
                println!("connection from: {:?}", remote);
                let handler2 = spawn!(dispatcher! {
                    Line, Line(s) => {
                        send!(printer, format!("recieved {} from {}:{}", s, remote, port))
                    },
                    Closed, _ => done!()
                });
                spawn!(|cid| line_reader(cid, c, handler2));
            }
        }
    );
    
    d.spawn2(Box::new(move |cid| listener(cid, [127, 0, 0, 1].into(), 1337, handler)));
    d.run();
}
