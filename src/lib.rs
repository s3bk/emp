#![feature(generators, generator_trait, get_type_id, const_type_id)]

extern crate bincode;
extern crate serde;

pub use std::any::{TypeId};
use std::ops::Generator;
use std::collections::{HashMap, HashSet, VecDeque};
use std::mem;
use std::rc::Rc;
use std::cell::RefCell;
use std::ops::GeneratorState;

pub mod message;
use message::*;

#[derive(Debug, Copy, Clone)]
pub struct Cid(u32);

pub type SpawnFunc = Box<Fn(Cid, Inbox) -> GenBox>;

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
type GenBox = Box<Generator<Yield=ProcessYield, Return=ProcessExit>>;
struct Process {
    generator: GenBox,
    inbox: Inbox,
    addr: Cid
}
impl Process {
    fn new(generator: GenBox, inbox: Inbox, addr: Cid) -> Process {
        Process {
            generator,
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

pub struct ExitReason {
    code: i32,
    msg: &'static str
}
#[derive(Debug)]
pub struct Sleep;

pub enum ProcessYield {
    Empty, /// the coroutine has nothing to do
    Send(Cid, Envelope),
    YieldTo(Cid, Envelope),
    Spawn(SpawnFunc)
}
pub enum ProcessExit {
    Done,
    Terminate(ExitReason),
}

#[thread_local] static mut MAX_ID: u32 = 0;
pub fn bump_id() -> u32 {
    let id = MAX_ID;
    MAX_ID += 1;
    id
}

pub struct PreparedCoro {
    cid: Cid,
    process: Process
}
impl PreparedCoro {
    pub fn cid(&self) -> Cid {
        self.cid
    }
}

pub struct Dispatcher {
    processes: HashMap<u32, Process>,
    ready: HashSet<u32>,
    ready2: Option<HashSet<u32>>,
    exit: Option<ExitReason>
}
impl Dispatcher {
    pub fn new() -> Dispatcher {
        Dispatcher {
            processes: HashMap::new(),
            ready: HashSet::new(),
            ready2: Some(HashSet::new()),
            exit: None
        }
    }
    pub fn spawn<F>(&mut self, func: F) -> Cid where
        F: Fn(Cid, Inbox) -> GenBox
    {
        let coro = Self::prepare_spawn(f);
        let cid = coro.cid();
        self.spawn_prepared(coro);
        cid
    }
    pub fn prepare_spawn<F>(f: F) -> PreparedCoro where F: Fn(Cid, Inbox) -> GenBox {
        let cid = Cid(bump_id());
        let inbox = Inbox::new();
        let process = Process::new(func(addr, inbox.clone()), inbox, addr);
        PreparedCoro { cid, process } 
    }
    fn spawn_prepared(&mut self, p: PreparedCoro) {
        let PreparedCoro { cid, process } = p;
        assert!(self.processes.insert(cid.0, process).is_none());
    }
    pub fn send(&mut self, addr: Cid, msg: Envelope) {
        println!("send {:?} to {:?}", msg, addr);
        self.processes.get_mut(&addr.0).unwrap().queue(msg);
        self.ready.insert(addr.0);
    }
    fn resume(&mut self, id: u32) -> GeneratorState<ProcessYield, ProcessExit> {
        let process = self.processes.get_mut(&id).unwrap();
        unsafe {
            process.generator.resume()
        }
    }
    fn yield_to(&mut self, mut addr: Cid, msg: Envelope) {
        self.send(addr, msg);
        self.run_one(addr.0);
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
                Yielded(ProcessYield::Spawn(f)) => 
                Yielded(ProcessYield::Empty) => break,
                Complete(ProcessExit::Terminate(reason)) => self.exit = Some(reason),
                Complete(ProcessExit::Done) => {
                    println!("{} terminated", proc_id);
                    self.processes.remove(&proc_id);
                    break;
                },
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
    
    pub fn run(&mut self) -> ExitReason {
        loop {
            // we have CPU work left
            while self.ready.len() > 0 {
                self.run_once();
            }
            
            // a chance to exit here
            if let Some(reason) = self.exit.take() {
                return reason;
            }
            
            // no CPU work left and not exiting, so we have to wait
            self.yield_to(Cid(0), Envelope::pack(Sleep));
        }
    }
}

