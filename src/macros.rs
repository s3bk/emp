/// send!(cid, message)
///
/// Send a message to the coroutine identified by cid.
/// Suspends the current coroutine.
#[macro_export]
macro_rules! send {
    ($addr:expr, $msg:expr) => (yield ProcessYield::Send($addr, Envelope::pack($msg)));  
    ($msg:expr => $addr:expr) => (yield ProcessYield::Send($addr, Envelope::pack($msg)));
}

/// yield_to!(cid, message)
///
/// Send a message to the coroutine identified by cid, and switch execution to it.
/// Suspends the current coroutine.
#[macro_export]
macro_rules! yield_to {
    ($addr:expr, $msg:expr) => (yield ProcessYield::YieldTo($addr, Envelope::pack($msg)));  
    ($msg:expr => $addr:expr) => (yield ProcessYield::YieldTo($addr, Envelope::pack($msg)));
}

/// recieve messages and handle the specified types
///
/// ```
/// recv!(envelope => {
///     // type,    pattern => { block },
///     (u32, u32), (a, b) => { code … }
///     String, s => println!("recieved {}", s)
/// }
/// ```
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

/// spawn a coroutine.
///
/// `spawn!(code)`
#[macro_export]
macro_rules! spawn {
    ($gen:block) => ({
        let coro = Dispatcher::prepare_spawn($gen);
        let cid = coro.cid();
        yield ProcessYield::Spawn(coro);
        cid
    })
}

/// create an event dispatcher
#[macro_export]
macro_rules! dispatcher {
    ($( $t:ty, $s:pat => $b:expr ),*) => ({
        move |_, inbox| move || loop {
            while let Some(e) = inbox.get() {
                recv!(e => { $( $t, $s => $b ),*  })
            }
            
            yield ProcessYield::Empty;
        }
    });
}

/// request to terminate the programm (not just the current coroutine)
#[macro_export]
macro_rules! exit {
    ($msg:expr) => (exit!(0, $msg));
    ($code:expr, $msg:expr) => (return ProcessExit::Terminate(
        ExitReason {
            code: $code,
            msg: $msg
        }
    ))
}
