#[macro_export]
macro_rules! no_msg {
    (yield $e:expr) => ({
        let e: Option<$crate::message::Envelope> = yield $e;
        assert!(e.is_none());
    })
}

/// send!(cid, message)
///
/// Send a message to the coroutine identified by cid.
/// Suspends the current coroutine.
#[macro_export]
macro_rules! send {
    ($addr:expr, $msg:expr) => (no_msg!(yield $crate::dispatch::ProcessYield::Send($addr, $crate::message::Envelope::pack($msg))));
    ($msg:expr => $addr:expr) => (no_msg!(yield $crate::dispatch::ProcessYield::Send($addr, $crate::message::Envelope::pack($msg))));
}

/// yield_to!(cid, message)
///
/// Send a message to the coroutine identified by cid, and switch execution to it.
/// Suspends the current coroutine.
#[macro_export]
macro_rules! yield_to {
    ($addr:expr, $msg:expr) => (no_msg!(yield $crate::dispatch::ProcessYield::YieldTo($addr, $crate::message::Envelope::pack($msg))));  
    ($msg:expr => $addr:expr) => (no_msg!(yield $crate::dispatch::ProcessYield::YieldTo($addr, $crate::message::Envelope::pack($msg))));
}

#[macro_export]
macro_rules! io {
    () => (no_msg!(yield $crate::dispatch::ProcessYield::Io))
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
    {$( $t:ty, $s:pat => $b:expr ),*  } => ({
        use std::any::TypeId;
        while let Some(envelope) = (yield $crate::dispatch::ProcessYield::Empty) {
            match envelope.type_id {
                $( id if id == TypeId::of::<$t>() => {
                    let $s: $t = envelope.unpack();
                    $b;
                } )*,
                _ => {}
            }
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
        no_msg!(yield $crate::dispatch::ProcessYield::Spawn(coro));
        cid
    });
}

/// create an event dispatch
#[macro_export]
macro_rules! dispatcher {
    ($( $t:ty, $s:pat => $b:expr ),*) => ({
        Dispatcher::prepare_spawn(move |_| move |_: Option<$crate::message::Envelope>| {
            recv!{ $( $t, $s => $b ),* }
            ProcessExit::Done
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
