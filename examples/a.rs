#![feature(generators)]

#[macro_use] extern crate msg;
use msg::*;

#[derive(Debug)]
struct Foo;

#[derive(Debug)]
struct Bar(u32);

#[derive(Debug)]
struct Baz(String);


/*
fn spawn_test() -> Process {
    let mut any = 0;
    let mut bar = 0;
    
    Process::new(move || {
        loop {
            const FOO_ID: TypeId = TypeId::of::<Foo>();
            const BAR_ID: TypeId = TypeId::of::<Bar>();
            while let Some(e) = self.inbox.get() {
                match e.type_id {
                    FOO_ID => {
                        let _ = *downcast::<Foo>(e);
                        println!("got a Foo");
                        any += 1;
                    },
                    BAR_ID => {
                        let Bar(n) = *downcast::<Bar>(e);
                        
                        bar += n;
                        println!("now {} bar", bar);
                        send!(1, format!("{} bars", bar));
                    },
                    _ => {}
                }
            }
            yield ProcessYield::Empty;
        }
    })
}*/
fn main() {
    let mut d = Dispatcher::new();
    let printer = d.spawn(|_, inbox| move || loop {
        while let Some(e) = inbox.get() {
            recv!(e => {
                String, s => { 
                    println!("printer: {}", s);
                }
            })
        }
        
        yield ProcessYield::Empty;
    });
    
    let mut any = 0;
    let mut bar = 0;
    let test = d.spawn(|_, inbox| move || loop {
        while let Some(e) = inbox.get() {
            recv!(e => {
                Foo, _ => {
                    println!("got a Foo");
                    any += 1;
                },
                Bar, Bar(n) => {
                    bar += n;
                    println!("now {} bar", bar);
                    
                    yield_to!(printer, format!("{} bars", bar));
                }
            })
        }
        
        yield ProcessYield::Empty;
    });
    
    d.send(test, Envelope::pack(Foo));
    d.send(test, Envelope::pack(Bar(42)));
    d.run();
}
