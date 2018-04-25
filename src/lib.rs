#![feature(generators, generator_trait, get_type_id, const_type_id)]

pub use std::any::{TypeId};
use std::ops::Generator;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::{self, Debug};
use std::mem;
use std::rc::Rc;
use std::cell::RefCell;
use std::ops::GeneratorState;

pub trait Message: Debug {}
impl<T: Debug> Message for T {}

pub struct Envelope {
    event: Box<Message>,
    pub type_id: TypeId
}
impl Envelope {
    pub fn pack<T: Message + 'static>(e: T) -> Envelope {
        Envelope {
            event: Box::new(e),
            type_id: TypeId::of::<T>()
        }
    }
    pub fn unpack<T: Message + 'static>(self) -> T {
        let Envelope { event, type_id } = self;
        assert_eq!(type_id, TypeId::of::<T>());
        
        unsafe {
            let ptr = Box::into_raw(event);
            *Box::from_raw(ptr as *mut T)
        }
    }
}
impl Debug for Envelope {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.event.fmt(f)
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Address(u32);

pub enum ProcessYield {
    Send(Address, Envelope),
    YieldTo(Address, Envelope),
    Empty
}
#[derive(Clone)]
pub struct Inbox {
    inner: Rc<RefCell<VecDeque<Envelope>>>
}
impl Inbox {
    fn new() -> Inbox {
        Inbox {
            inner: Rc::new(RefCell::new(VecDeque::new()))
        }
    }
    pub fn get(&self) -> Option<Envelope> {
        self.inner.borrow_mut().pop_front()
    }
    fn put(&self, msg: Envelope) {
        self.inner.borrow_mut().push_back(msg);
    }
}
struct Process {
    generator: Box<Generator<Yield=ProcessYield, Return=()>>,
    inbox: Inbox,
    addr: Address
}
impl Process {
    fn new<G>(gen: G, inbox: Inbox, addr: Address) -> Process where G: Generator<Yield=ProcessYield, Return=()> + 'static {
        Process {
            generator: Box::new(gen) as _,
            inbox,
            addr
        }
    }
    fn queue(&mut self, msg: Envelope) {
        self.inbox.put(msg);
    }
}

#[macro_export]
macro_rules! send {
    ($addr:expr, $msg:expr) => (yield ProcessYield::Send($addr, Envelope::pack($msg)));  
    ($msg:expr => $addr:expr) => (yield ProcessYield::Send($addr, Envelope::pack($msg)));
}
#[macro_export]
macro_rules! yield_to {
    ($addr:expr, $msg:expr) => (yield ProcessYield::YieldTo($addr, Envelope::pack($msg)));  
    ($msg:expr => $addr:expr) => (yield ProcessYield::YieldTo($addr, Envelope::pack($msg)));
}
#[macro_export]
macro_rules! recv {
    ( $e:expr => {$( $t:ty, $s:pat => $b:expr ),*  } ) => ({
        let e: Envelope = $e;
        match e.type_id {
            $( id if id == TypeId::of::<$t>() => {
                let $s: $t = e.unpack();
                $b
            } )*,
            _ => {}
        }
    })
}


pub struct Dispatcher {
    processes: HashMap<u32, Process>,
    ready: HashSet<u32>,
    ready2: Option<HashSet<u32>>,
    max_id: u32
}
impl Dispatcher {
    pub fn new() -> Dispatcher {
        Dispatcher {
            processes: HashMap::new(),
            ready: HashSet::new(),
            ready2: Some(HashSet::new()),
            max_id: 0
        }
    }
    pub fn spawn<F, G>(&mut self, func: F) -> Address where
        F: Fn(Address, Inbox) -> G,
        G: Generator<Yield=ProcessYield, Return=()> + 'static
    {
        let addr = Address(self.max_id);
        self.max_id += 1;
        
        let inbox = Inbox::new();
        let process = Process::new(func(addr, inbox.clone()), inbox, addr);
        self.processes.insert(addr.0, process);
        addr
    }
    pub fn send(&mut self, addr: Address, msg: Envelope) {
        println!("send {:?} to {:?}", msg, addr);
        self.processes.get_mut(&addr.0).unwrap().queue(msg);
        self.ready.insert(addr.0);
    }
    fn resume(&mut self, id: u32) -> GeneratorState<ProcessYield, ()> {
        let process = self.processes.get_mut(&id).unwrap();
        unsafe {
            process.generator.resume()
        }
    }
    fn run_one(&mut self, mut proc_id: u32) {
        use std::ops::GeneratorState::*;
        println!("running {}", proc_id);
        
        loop {
            match self.resume(proc_id) {
                Yielded(ProcessYield::Send(addr, msg)) => self.send(addr, msg),
                Yielded(ProcessYield::YieldTo(addr, msg)) => {
                    self.send(addr, msg);
                    
                    // execute id now
                    proc_id = addr.0;
                    println!("yield to {:?}", addr);
                }
                Yielded(ProcessYield::Empty) => break,
                Complete(_) => {
                    println!("{} terminated", proc_id);
                    self.processes.remove(&proc_id);
                    break;
                }
            }
        }
    }
        
    fn run_once(&mut self) {
        let mut ready = self.ready2.take().unwrap();
        mem::swap(&mut self.ready, &mut ready);
        // self.ready is now empty, ready contains process we need to run
        
        for id in ready.drain() {
            self.run_one(id);
        }
        
        // put empty hashset back
        self.ready2 = Some(ready);
    }
    
    pub fn run(&mut self) {
        while self.ready.len() > 0 {
            self.run_once();
        }
    }
}

