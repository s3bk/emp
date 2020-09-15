pub use std::any::{TypeId};
use std::ops::Generator;
use std::mem;
use std::ops::GeneratorState;
use std::collections::{HashMap, HashSet, VecDeque};
use std::pin::Pin;
use std::future::Future;
use std::task::{Context, Waker, Poll};
use crate::message::*;
use crate::epoll;
use slotmap::{SlotMap, new_key_type, KeyData};
use crossbeam::channel::{unbounded, Receiver, Sender};

/// unique identifier for each coroutine
#[derive(Debug, Copy, Clone)]
pub struct Cid(ProcessKey);
impl Cid {
    pub fn as_ffi(self) -> u64 {
        KeyData::from(self.0).as_ffi()
    }
    pub fn from_ffi(data: u64) -> Self {
        Cid(KeyData::from_ffi(data).into())
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
    
    /// spawn a coroutine (to be used with `Dispatcher::prepare_spawn`)
    Spawn(GenBox), 
    Spawn2(SpawnBox), 

    SpawnFut(FutBox),
    
    /// waiting for IO
    Io
}


#[derive(Debug)]
pub enum ResumeArg {
    Empty,

    Message(Envelope),

    Spawned(Cid),
}

/// return type for coroutines
pub enum ProcessExit {
    /// control flow reached the end
    Done,
    
    /// we want the whole program to termiante
    Terminate(ExitReason)
}

/// actual generator when running
pub type GenBox = Pin<Box<dyn Generator<ResumeArg, Yield=ProcessYield, Return=ProcessExit>>>;
pub type FutBox = Pin<Box<dyn Future<Output=(Cid, Envelope)>>>;
pub type SpawnBox = Box<dyn FnOnce(Cid) -> GenBox>;

struct Process {
    generator: GenBox,
}

pub struct PreparedCoro {
    cid: ProcessKey,
    process: Process
}
impl PreparedCoro {
    pub fn cid(&self) -> Cid {
        Cid(self.cid)
    }
}

new_key_type! {
    struct FutureKey;
    struct ProcessKey;
}

pub struct Dispatcher {
    processes: SlotMap<ProcessKey, Process>,
    futures: SlotMap<FutureKey, (FutBox, Waker)>,
    queue: VecDeque<(ProcessKey, ResumeArg)>,
    queue2: Option<VecDeque<(ProcessKey, ResumeArg)>>,
    exit: Option<ExitReason>,
    wake_rx: Option<Receiver<FutureKey>>,
    wake_tx: Sender<FutureKey>,
    sleeper: Option<ProcessKey>,
}
impl Dispatcher {
    pub fn new() -> Dispatcher {
        let (wake_tx, wake_rx) = unbounded();
        let mut d = Dispatcher {
            processes: SlotMap::with_key(),
            futures: SlotMap::with_key(),
            queue: VecDeque::new(),
            queue2: Some(VecDeque::new()),
            exit: None,
            wake_rx: Some(wake_rx),
            wake_tx,
            sleeper: None,
        };
        let s = d.spawn(epoll::sleeper());
        d.sleeper = Some(s.0);
        d
    }

    pub fn spawn2(&mut self, f: Box<dyn FnOnce(Cid) -> GenBox>) -> Cid {
        self.spawn3(f)
    }

    pub fn spawn(&mut self, generator: GenBox) -> Cid {
        self.spawn3(move |_| generator)
    }

    fn spawn3(&mut self, f: impl FnOnce(Cid) -> GenBox) -> Cid {
        Cid(self.processes.insert_with_key(|key| {
            let mut generator = f(Cid(key));
            generator.as_mut().resume(ResumeArg::Empty);

            Process {
                generator,
            }
        }))
    }

    fn spawn_fut(&mut self, fut: FutBox) {
        let tx = self.wake_tx.clone();
        self.futures.insert_with_key(|key| {
            let waker = DispatchWaker {
                tx,
                key
            };
            let waker = unsafe {
                Waker::from_raw(Arc::new(waker).into())
            };
            (fut, waker)
        });
    }

    pub fn send(&mut self, addr: Cid, msg: Envelope) {
        //println!("send {:?} to {:?}", msg, addr);
        self.queue.push_back((addr.0, ResumeArg::Message(msg)));
    }

    fn run_one(&mut self, proc_id: ProcessKey, arg: ResumeArg) {
        let mut next_arg = Some(arg);
        
        while let Some(arg) = next_arg.take() {
            let process = match self.processes.get_mut(proc_id) {
                None => return,
                Some(p) => p,
            };
            
            //println!("running {:?}({:?})", proc_id, arg);
            let state = process.generator.as_mut().resume(arg);
            match state {
                GeneratorState::Yielded(y) => match y { 
                    ProcessYield::Send(addr, msg) => {
                        self.send(addr, msg);
                    }
                    ProcessYield::Spawn(coro) => {
                        let cid = self.spawn(coro);
                        next_arg = Some(ResumeArg::Spawned(cid));
                    }
                    ProcessYield::Spawn2(f) => {
                        let cid = self.spawn2(f);
                        next_arg = Some(ResumeArg::Spawned(cid));
                    }
                    ProcessYield::SpawnFut(fut) => {
                        self.spawn_fut(fut);
                    }
                    ProcessYield::Empty => return,
                    ProcessYield::Io => return,
                },
                GeneratorState::Complete(e) => {
                    //println!("{} terminated", &proc_id);
                    self.processes.remove(proc_id);
                    match e {
                        ProcessExit::Terminate(reason) => self.exit = Some(reason),
                        ProcessExit::Done => {}
                    }
                    return;
                }
            }
        }

        self.queue.push_back((proc_id, ResumeArg::Empty));
    }
    
    fn run_once(&mut self) {
        let mut queue = self.queue2.take().unwrap();
        mem::swap(&mut self.queue, &mut queue);
        // self.ready is now empty, ready contains process we need to run
        
        for (id, arg) in queue.drain(..) {
            self.run_one(id, arg);
        }
        
        // put empty hashset back
        self.queue2 = Some(queue);
    }
    
    pub fn run(&mut self) -> ExitReason {
        loop {
            // we have CPU work left
            while self.queue.len() > 0 {
                self.run_once();
            }
            
            // a chance to exit here
            if let Some(reason) = self.exit.take() {
                return reason;
            }
            
            // no CPU work left and not exiting, so we have to wait
            if let Some(sleeper) = self.sleeper {
                self.run_one(sleeper, ResumeArg::Message(Envelope::pack(Sleep)));
            } else {
                return ExitReason {
                    code: 0,
                    msg: "no sleeper"
                };
            }
        }
    }
}

use std::task::{Wake, RawWaker};
use std::sync::Arc;

struct DispatchWaker {
    tx: Sender<FutureKey>,
    key: FutureKey,
}
impl Wake for DispatchWaker {
    fn wake(self: Arc<Self>) {
        self.tx.send(self.key).unwrap();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.tx.send(self.key).unwrap();
    }
}
