#[macro_export]
macro_rules! no_msg {
    (yield $e:expr) => ({
        match (yield $e) {
            $crate::dispatch::ResumeArg::Empty => (),
            _ => unreachable!()
        }
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
        loop {
            match (yield $crate::dispatch::ProcessYield::Empty) {
                $crate::dispatch::ResumeArg::Message(envelope) => {
                    match envelope.type_id {
                        $( id if id == TypeId::of::<$t>() => {
                            let $s: $t = envelope.unpack();
                            $b;
                        } )*,
                        _ => {}
                    }
                },
                $crate::dispatch::ResumeArg::Empty => break,
                _ => unreachable!()
            }
        }
    })
}

/// spawn a coroutine.
///
/// `spawn!(code)`
#[macro_export]
macro_rules! spawn {
    (|$cid:ident| $coro:expr) => {
        match (yield $crate::dispatch::ProcessYield::Spawn2(Box::new(move |$cid: $crate::dispatch::Cid| $coro))) {
            $crate::dispatch::ResumeArg::Spawned(cid) => cid,
            _ => unreachable!()
        }
    };
    ($coro:expr) => ({
        let coro = $coro;
        match (yield $crate::dispatch::ProcessYield::Spawn(coro)) {
            $crate::dispatch::ResumeArg::Spawned(cid) => cid,
            _ => unreachable!()
        }
    });
}

/// create an event dispatch
#[macro_export]
macro_rules! dispatcher {
    ($( $t:ty, $s:pat => $b:expr ),*) => ({
        Box::pin(Box::new(move |_: $crate::dispatch::ResumeArg| {
            recv!{ $( $t, $s => $b ),* }
            ProcessExit::Done
        }))
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
