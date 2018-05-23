/// send!(cid, message)
///
/// Send a message to the coroutine identified by cid.
/// Suspends the current coroutine.
#[macro_export]
macro_rules! send {
    ($addr:expr, $msg:expr) => (yield $crate::dispatch::ProcessYield::Send($addr, $crate::message::Envelope::pack($msg)));  
    ($msg:expr => $addr:expr) => (yield $crate::dispatch::ProcessYield::Send($addr, $crate::message::Envelope::pack($msg)));
}

/// yield_to!(cid, message)
///
/// Send a message to the coroutine identified by cid, and switch execution to it.
/// Suspends the current coroutine.
#[macro_export]
macro_rules! yield_to {
    ($addr:expr, $msg:expr) => (yield $crate::dispatch::ProcessYield::YieldTo($addr, $crate::message::Envelope::pack($msg)));  
    ($msg:expr => $addr:expr) => (yield $crate::dispatch::ProcessYield::YieldTo($addr, $crate::message::Envelope::pack($msg)));
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
        use std::any::TypeId;
        let e: $crate::message::Envelope = $e;
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
    ($coro:expr) => ({
        let coro = $coro;
        let cid = coro.cid();
        yield $crate::dispatch::ProcessYield::Spawn(coro);
        cid
    });
}

/// create an event dispatch
#[macro_export]
macro_rules! dispatcher {
    ($( $t:ty, $s:pat => $b:expr ),*) => ({
        Dispatcher::prepare_spawn(move |_, inbox| move || loop {
            while let Some(e) = inbox.get() {
                recv!(e => { $( $t, $s => $b ),*  })
            }
            
            yield $crate::dispatch::ProcessYield::Empty;
        })
    });
}

/// request to terminate the programm (not just the current coroutine)
#[macro_export]
macro_rules! exit {
    ($msg:expr) => (exit!(0, $msg));
    ($code:expr, $msg:expr) => (return $crate::dispatch::ProcessExit::Terminate(
        $crate::dispatch::ExitReason {
            code: $code,
            msg: $msg
        }
    ))
}

#[macro_export]
macro_rules! done {
    () => (return $crate::dispatch::ProcessExit::Done)
}
