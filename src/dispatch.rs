pub use std::any::{TypeId};
use std::ops::Generator;
use std::mem;
use std::ops::GeneratorState;
use std::collections::{HashMap, HashSet, VecDeque};
use std::pin::Pin;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::future::Future;
use std::task::{Context, Waker, Poll};
use crate::message::*;
use crate::epoll;
use slotmap::{SlotMap, new_key_type};

/// unique identifier for each coroutine
#[derive(Debug, Copy, Clone)]
pub struct Cid(pub u32);

/// message inbox of each coroutine
pub struct Inbox {
    inner: VecDeque<Envelope>
}
impl Inbox {
    fn new() -> Inbox {
        Inbox {
            inner: VecDeque::new()
        }
    }
    pub fn get(&mut self) -> Option<Envelope> {
        self.inner.pop_front()
    }
    fn put(&mut self, msg: Envelope) {
        self.inner.push_back(msg);
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
pub type GenBox = Pin<Box<dyn Generator<Option<Envelope>, Yield=ProcessYield, Return=ProcessExit>>>;
pub type FutBox = Pin<Box<dyn Future<Output=(Cid, Envelope)>>>;
struct Process {
    generator: GenBox,
    inbox: Inbox,
    empty: bool
}
impl Process {
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

new_key_type! {
    struct FutureKey;
}

pub struct Dispatcher {
    processes: HashMap<u32, Process>,
    futures: SlotMap<FutureKey, (FutBox, Waker)>,
    ready: HashSet<u32>,
    ready2: Option<HashSet<u32>>,
    exit: Option<ExitReason>,
    wake_rx: Receiver<u32>,
    wake_tx: Sender<u32>
}
impl Dispatcher {
    pub fn new() -> Dispatcher {
        let (wake_tx, wake_rx) = channel();
        let mut d = Dispatcher {
            processes: HashMap::new(),
            futures: SlotMap::with_key(),
            ready: HashSet::new(),
            ready2: Some(HashSet::new()),
            exit: None,
            wake_rx,
            wake_tx
        };
        let s = d.spawn(epoll::sleeper());
        d
    }
    pub fn prepare_spawn<F, G>(func: F) -> PreparedCoro where
        F: FnOnce(Cid) -> G,
        G: Generator<Option<Envelope>, Yield=ProcessYield, Return=ProcessExit> + 'static
    {
        let cid = Cid(bump_id());
        let inbox = Inbox::new();
        let gen = Box::pin(func(cid));
        let process = Process {
            generator: gen as GenBox,
            inbox,
            empty: false
        };
        PreparedCoro { cid, process }
    }
    pub fn spawn(&mut self, p: PreparedCoro) -> Cid {
        let PreparedCoro { cid, process } = p;
        assert!(self.processes.insert(cid.0, process).is_none());
        cid
    }

    pub fn send(&mut self, addr: Cid, msg: Envelope) {
        println!("send {:?} to {:?}", msg, addr);
        self.processes.get_mut(&addr.0).unwrap().queue(msg);
        self.ready.insert(addr.0);
    }

    fn poll_future(&mut self, key: FutureKey) {
        let (future, waker) = self.futures.get_mut(key).expect("no such future");
        let mut context = Context::from_waker(waker);
        match future.as_mut().poll(&mut context) {
            Poll::Ready((cid, msg)) => {
                self.futures.remove(key);
                self.yield_to(cid, msg);
            }
            Poll::Pending => {
                info!("future {:?}: spurius wakeup", key);
            }
        }
    }

    fn resume(&mut self, id: u32) -> Option<GeneratorState<ProcessYield, ProcessExit>> {
        let process = match self.processes.get_mut(&id) {
            None => return None,
            Some(p) => p,
        };
        
        // we only deliver an envelope if the process asked for it.
        // otherwise it would get lost anyway
        let msg = match process.empty {
            true => match process.inbox.get() {
                Some(msg) => Some(msg),
                None => return None, // if the process wants mail but we have non, return None to break the loop
            },
            false => None
        };
        let r = process.generator.as_mut().resume(msg);
        process.empty = match r {
            GeneratorState::Yielded(ProcessYield::Empty) => true,
            _ => false
        };
        Some(r)
    }
    fn yield_to(&mut self, addr: Cid, msg: Envelope) {
        self.send(addr, msg);
        self.run_one(addr.0);
    }
    fn run_one(&mut self, mut proc_id: u32) {
        println!("running {}", proc_id);
        
        while let Some(state) = self.resume(proc_id) {
            match state {
                GeneratorState::Yielded(y) => match y { 
                    ProcessYield::Send(addr, msg) => self.send(addr, msg),
                    ProcessYield::YieldTo(addr, msg) => {
                        self.send(addr, msg);
                        
                        // execute id now
                        proc_id = addr.0;
                        println!("yield to {:?}", addr);
                    }
                    ProcessYield::Spawn(coro) => {
                        self.spawn(coro);
                    }
                    ProcessYield::Empty => continue,
                    ProcessYield::Io => break,
                },
                GeneratorState::Complete(e) => {
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
