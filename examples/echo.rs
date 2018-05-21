#![feature(generators)]

#[macro_use] extern crate msg;
use msg::*;

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
    
    let manager = d.spawn(dispatcher! {
        Connection, c => { 
            println!("connection from: {:?}", c.remote());
                listener!(c => conn_handler)
            }
            let handler = spawn!(move || loop {
            
            });
        }
    });
    
    let mut any = 0;
    let mut bar = 0;
    let test = d.spawn(dispatcher! {
        Foo, _ => {
            println!("got a Foo");
            any += 1;
        },
        Bar, Bar(n) => {
            bar += n;
            println!("now {} bar", bar);
            
            yield_to!(printer, format!("{} bars", bar));
        }
    });
    
    d.send(test, Envelope::pack(Foo));
    d.send(test, Envelope::pack(Bar(42)));
    d.run();
}
