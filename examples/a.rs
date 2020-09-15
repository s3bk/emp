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
    let sleeper = d.spawn(dispatcher!{
        Sleep, _ => exit!("done")
    });
    let printer = d.spawn(dispatcher!{
        String, s => { 
            println!("printer: {}", s);
        }
    });
    
    let mut any = 0;
    let mut bar = 0;
    let test = d.spawn(dispatcher!{
        Foo, _ => {
            println!("got a Foo");
            any += 1;
        },
        Bar, Bar(n) => {
            bar += n;
            println!("now {} bar", bar);
            
            send!(printer, format!("{} bars", bar))
        }
    });
    
    d.send(test, Envelope::pack(Foo));
    d.send(test, Envelope::pack(Bar(42)));
    d.run();
}
