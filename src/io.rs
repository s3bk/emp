use ffi;
use std::time::Duration;
use std::{ptr, cmp};
use syscall_alt::syscalls::Syscall;
use libc;

#[derive(Debug)]
pub enum AIoError {
    EventLimit,
    NotInitialized,
    KernelOutOfMemory,
    KernelRessources,
    NotImplemented,
    BadFileDescriptor,
    Interrupted
}

#[derive(Debug)]
pub enum ReadError {
    NotSubmitted,
    Taken,
    Errno(i32)
}

pub struct AIoContext {
    id:         u64,
    capacity:   usize
}
impl AIoContext {
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn setup(nr_events: usize) -> Result<AIoContext, AIoError> {
        let mut id: ffi::aio_context_t = 0;
        let res = unsafe {
            Syscall::io_setup.syscall2(
                nr_events as isize,
                &mut id as *mut ffi::aio_context_t as isize
            )
        };
        if res == 0 {
            Ok(AIoContext { id: id, capacity: nr_events })
        } else {
            match -res as i32 {
                libc::EAGAIN => Err(AIoError::EventLimit),
                libc::EFAULT => panic!("internal error (invalid pointer)"),
                libc::EINVAL => Err(AIoError::NotInitialized),
                libc::ENOMEM => Err(AIoError::KernelOutOfMemory),
                libc::ENOSYS => Err(AIoError::NotImplemented),
                e => panic!("unknown return code {}", e)
            }
        }
    }
    pub fn destroy(self) {
        let res = unsafe {
            Syscall::io_destroy.syscall1(self.id as isize)
        };
        match -res as i32 {
            0 => (),
            libc::EFAULT | libc::EINVAL => panic!("invalid context"),
            libc::ENOSYS => panic!("attempt to destroy a context, that shouldn't exist"),
            e => panic!("unknown return code {}", e)
        }
    }
    /** The caller has to ensure the buffers contained in the iocbs
        are not dropped until all jobs have finished */
    pub unsafe fn submit(&self, iocbs: &[*const ffi::Iocb])
     -> Result<(), AIoError>
    {
        assert!(iocbs.len() <= self.capacity);
        let res = Syscall::io_submit.syscall3(
            self.id as isize,
            iocbs.len() as isize,
            iocbs.as_ptr() as isize
        );
        if res as usize == iocbs.len() {
            Ok(())
        } else {
            match -res as i32 {
                libc::EAGAIN => Err(AIoError::KernelRessources),
                libc::EBADF => Err(AIoError::BadFileDescriptor),
                libc::EFAULT => panic!("internal error (invalid data)"),
                libc::EINVAL => Err(AIoError::NotInitialized),
                libc::ENOSYS => Err(AIoError::NotImplemented),
                e => panic!("unknown return code {}", e)
            }
        }
    }
    
    
    pub fn get_events(&self, min: usize, buf: &mut Vec<ffi::Event>, timeout: Option<Duration>)
     -> Result<(), AIoError> {
        let max = cmp::min(self.capacity, buf.capacity());
        assert!(min <= max);
        
        let timeout = timeout.as_ref().map(|t| libc::timespec {
            tv_sec:  t.as_secs() as libc::time_t,
            tv_nsec: t.subsec_nanos() as libc::c_long
        });
        let timeout_p = match timeout {
            Some(ref t) => t as *const libc::timespec,
            None => ptr::null()
        };
        
        let res = unsafe {
            Syscall::io_getevents.syscall5(
                self.id as isize,
                min as isize,
                max as isize,
                buf.as_mut_ptr() as isize,
                timeout_p as isize
            )
        };
        
        if res >= 0 {
            unsafe { buf.set_len(res as usize) };
            Ok(())
        } else {
            match -res as i32 {
                libc::EINTR => Err(AIoError::Interrupted),
                libc::EFAULT => panic!("internal error: invalid events or timeout"),
                libc::EINVAL => panic!("internal error: ctx invalid or out of range"),
                libc::ENOSYS => Err(AIoError::NotImplemented),
                e => panic!("unknown return code {}", e)
            }
        }
    }
}
