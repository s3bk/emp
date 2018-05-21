pub use std::any::{TypeId};
use std::ops::Generator;
use std::mem;
use std::rc::Rc;
use std::cell::RefCell;
use std::ops::GeneratorState;
use std::collections::{HashMap, HashSet, VecDeque};
use message::*;

/// unique identifier for each coroutine
#[derive(Debug, Copy, Clone)]
pub struct Cid(pub u32);

/// message inbox of each coroutine
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

/// why we want to terminate
pub struct ExitReason {
    pub code: i32,
    pub msg: &'static str
}

/// internally used to signal that we are out of work.
#[derive(Debug)]
pub struct Sleep;

/// yield type for coroutines
pub enum ProcessYield {
    /// the coroutine has nothing to do
    Empty, 
    
    /// send a message to …
    Send(Cid, Envelope),
    
    /// send a message and switch execution to …
    YieldTo(Cid, Envelope),
    
    /// spawn a coroutine (to be used with `Dispatcher::prepare_spawn`)
    Spawn(PreparedCoro), 
    
    /// waiting for IO
    Io
}

/// return type for coroutines
pub enum ProcessExit {
    /// control flow reached the end
    Done,
    
    /// we want the whole program to termiante
    Terminate(ExitReason)
}

#[thread_local] static mut MAX_ID: u32 = 0;
fn bump_id() -> u32 {
    unsafe {
        let id = MAX_ID;
        MAX_ID += 1;
        id
    }
}

/// actual generator when running
pub type GenBox = Box<Generator<Yield=ProcessYield, Return=ProcessExit>>;
struct Process {
    generator: GenBox,
    inbox: Inbox
}
impl Process {
    fn new(generator: GenBox, inbox: Inbox) -> Process {
        Process {
            generator,
            inbox
        }
    }
    fn queue(&mut self, msg: Envelope) {
        self.inbox.put(msg);
    }
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
    pub fn spawn<F, G>(&mut self, func: F) -> Cid where
        F: FnOnce(Cid, Inbox) -> G,
        G: Generator<Yield=ProcessYield, Return=ProcessExit> + 'static
    {
        let coro = Self::prepare_spawn(func);
        let cid = coro.cid();
        self.spawn_prepared(coro);
        cid
    }
    pub fn prepare_spawn<F, G>(func: F) -> PreparedCoro where
        F: FnOnce(Cid, Inbox) -> G,
        G: Generator<Yield=ProcessYield, Return=ProcessExit> + 'static
    {
        let cid = Cid(bump_id());
        let inbox = Inbox::new();
        let process = Process::new(box func(cid, inbox.clone()) as GenBox, inbox);
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
    fn resume(&mut self, id: u32) -> Option<GeneratorState<ProcessYield, ProcessExit>> {
        self.processes
            .get_mut(&id)
            .map(|process| unsafe {
                process.generator.resume()
            })
    }
    fn yield_to(&mut self, addr: Cid, msg: Envelope) {
        self.send(addr, msg);
        self.run_one(addr.0);
    }
    fn run_one(&mut self, mut proc_id: u32) {
        use std::ops::GeneratorState::*;
        println!("running {}", proc_id);
        
        while let Some(state) = self.resume(proc_id) {
            match state {
                Yielded(y) => match y { 
                    ProcessYield::Send(addr, msg) => self.send(addr, msg),
                    ProcessYield::YieldTo(addr, msg) => {
                        self.send(addr, msg);
                        
                        // execute id now
                        proc_id = addr.0;
                        println!("yield to {:?}", addr);
                    }
                    ProcessYield::Spawn(coro) => self.spawn_prepared(coro),
                    ProcessYield::Empty => break,
                    ProcessYield::Io => break,
                },
                Complete(e) => {
                    println!("{} terminated", &proc_id);
                    self.processes.remove(&proc_id);
                    match e {
                        ProcessExit::Terminate(reason) => self.exit = Some(reason),
                        ProcessExit::Done => {}
                    }
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
