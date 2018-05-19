use std::ffi::{OsStr, CString};
use std::os::unix::io::{AsRawFd, RawFd};
use std::path::Path;
use std::rc::Rc;
use std::os::unix::ffi::OsStrExt;
use std::{mem, fmt};
use libc;
use syscall_alt::syscalls::{syscall1, syscall2, syscall3};
use syscall_alt::constants::SYS::*;

use common::*;
use io::ReadError;
use mem::Slice;

const O_DIRECT:     i32 = 0o00040000;
const O_DIRECTORY:  i32 = 0o00200000;
const O_PATH:       i32 = 0o10000000;

#[derive(Debug)]
pub enum OpenError {
    AccessNotAllowed,
    FileAlreadyExists(String),
    FilesystemUnsupported,
    IsDirectory(String),
    TooManySymlinks,
    TooManyOpenFilesInProcess,
    TooManyOpenFilesInSystem,
    NameTooLong,
    DoesNotExist(String),
    KernelOutOfMemory,
    NoSpaceLeftOnDevice,
    PathComponentNonDirectory,
    PermissionDenied,
    FilesystemReadOnly
}

pub struct OwnedDirectory {
    dirfd:  u32
}
impl OwnedDirectory {
    pub fn open(path: &OsStr) -> Result<OwnedDirectory, OpenError> {
        let fd = open(&path, O_DIRECTORY | O_PATH)?;
        Ok(OwnedDirectory { dirfd: fd })
    }
    pub fn file(&self, name: &OsStr) -> Result<SharedFile, OpenError> {
        let fd = openat(self.dirfd, name, O_DIRECT)?;
        Ok(SharedFile::from_fd(fd))
    }
    pub fn directory(&self, name: &OsStr) -> Result<OwnedDirectory, OpenError> {
        let fd = openat(self.dirfd, name, O_DIRECTORY | O_PATH)?;
        Ok(OwnedDirectory { dirfd: fd })
    }
}
impl Drop for OwnedDirectory {
    fn drop(&mut self) {
        unsafe {
            close(self.dirfd);
        }
    }
}
impl AsyncDirectory for OwnedDirectory {
    type File = SharedFile;
    type Error = OpenError;
    
    fn get_directory(&self, path: &str) -> Box<Future<Item=Self, Error=Self::Error>> {
        box future::result(self.directory(&OsStr::new(path)))
    }
    fn get_file(&self, path: &str) -> Box<Future<Item=Self::File, Error=Self::Error>> {
        box future::result(self.file(&OsStr::new(path)))
    }
}

#[derive(Clone)]
pub struct SharedDirectory {
    inner: Rc<OwnedDirectory>
}
impl AsyncDirectory for SharedDirectory {
    type File = SharedFile;
    type Error = OpenError;
    
    fn get_directory(&self, path: &str) -> Box<Future<Item=Self, Error=Self::Error>> {
        box self.inner.get_directory(path)
        .map(|dir| SharedDirectory { inner: Rc::new(dir) })
    }
    fn get_file(&self, path: &str) -> Box<Future<Item=Self::File, Error=Self::Error>> {
        self.inner.get_file(path)
    }
}

impl AsyncOpen for SharedDirectory {
    type Error = OpenError;
    fn open(name: &str) -> Box<Future<Item=Self, Error=Self::Error>> {
        box future::result(
            OwnedDirectory::open(name.as_ref()).map(|dir|
                SharedDirectory { inner: Rc::new(dir) }
            )
        )
    }
}

pub struct File {
    fd:     u32,
    stat:   libc::stat
}
impl fmt::Debug for File {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "OwnedFile {{ fd: {}, stat: ... }}", self.fd)
    }
}

impl AsRawFd for File {
    /// Return the raw underlying fd. The caller must make sure self's
    /// lifetime is longer than any users of the fd.
    fn as_raw_fd(&self) -> RawFd {
        self.fd as RawFd
    }
}

fn open(pathname: &OsStr, flags: i32) -> Result<u32, OpenError>
{
    let cname = CString::new(pathname.as_bytes()).unwrap();
    let res = unsafe {
        syscall2(SYS_open, cname.as_ptr() as isize, flags as isize)
    };
    if res >= 0 {
        assert!(res <= ::std::u32::MAX as isize);
        Ok(res as u32)
    } else {
        let name = cname.into_string().unwrap_or_else(|_| "<invalid utf-8>".into());
        match -res as i32 {
            libc::EACCES => Err(OpenError::AccessNotAllowed),
            libc::EEXIST => Err(OpenError::FileAlreadyExists(name)),
            libc::EINVAL => Err(OpenError::FilesystemUnsupported),
            libc::EISDIR => Err(OpenError::IsDirectory(name)),
            libc::ELOOP => Err(OpenError::TooManySymlinks),
            libc::EMFILE => Err(OpenError::TooManyOpenFilesInProcess),
            libc::ENFILE => Err(OpenError::TooManyOpenFilesInSystem),
            libc::ENAMETOOLONG => Err(OpenError::NameTooLong),
            libc::ENOENT => Err(OpenError::DoesNotExist(name)),
            libc::ENOMEM => Err(OpenError::KernelOutOfMemory),
            libc::ENOSPC => Err(OpenError::NoSpaceLeftOnDevice),
            libc::ENOTDIR => Err(OpenError::PathComponentNonDirectory),
            libc::EPERM => Err(OpenError::PermissionDenied),
            libc::EROFS => Err(OpenError::FilesystemReadOnly),
            e => panic!("error code: {}", e)
        }
    }
}

fn openat(dirfd: u32, pathname: &OsStr, flags: i32) -> Result<u32, OpenError>
{
    let cname = CString::new(pathname.as_bytes()).unwrap();
    let flags = flags | O_DIRECT;
    let res = unsafe {
        syscall3(SYS_openat, dirfd as isize, cname.as_ptr() as isize, flags as isize)
    };
    if res >= 0 {
        assert!(res <= ::std::u32::MAX as isize);
        Ok(res as u32)
    } else {
        let name = cname.into_string().unwrap_or_else(|_| "<invalid utf-8>".into());
        match -res as i32 {
            libc::EACCES => Err(OpenError::AccessNotAllowed),
            libc::EEXIST => Err(OpenError::FileAlreadyExists(name)),
            libc::EINVAL => Err(OpenError::FilesystemUnsupported),
            libc::EISDIR => Err(OpenError::IsDirectory(name)),
            libc::ELOOP => Err(OpenError::TooManySymlinks),
            libc::EMFILE => Err(OpenError::TooManyOpenFilesInProcess),
            libc::ENFILE => Err(OpenError::TooManyOpenFilesInSystem),
            libc::ENAMETOOLONG => Err(OpenError::NameTooLong),
            libc::ENOENT => Err(OpenError::DoesNotExist(name)),
            libc::ENOMEM => Err(OpenError::KernelOutOfMemory),
            libc::ENOSPC => Err(OpenError::NoSpaceLeftOnDevice),
            libc::ENOTDIR => Err(OpenError::PathComponentNonDirectory),
            libc::EPERM => Err(OpenError::PermissionDenied),
            libc::EROFS => Err(OpenError::FilesystemReadOnly),
            e => panic!("error code: {}", e)
        }
    }
}

unsafe fn close(fd: u32) {
    let ret = syscall1(SYS_close, fd as isize);
    assert_eq!(ret, 0);
}

/// fd has to be a valid file descriptor.
/// will panic if unsuccessful.
fn stat(fd: u32) -> libc::stat {
    let mut stat: libc::stat = unsafe { mem::zeroed() };
    let res = unsafe {
        syscall2(SYS_fstat, fd as isize, &mut stat as *mut libc::stat as isize)
    };
    if res == 0 {
        stat
    } else {
        panic!("stat({}) -> {}", fd, -res);
    }
}

impl File {
    unsafe fn from_fd(fd: u32) -> File {
        File { fd: fd, stat: stat(fd) }
    }
    pub fn open<P: AsRef<Path>>(path: P) -> Result<File, OpenError> {
        let fd = open(path.as_ref().as_os_str(), O_DIRECT)?;
        let file = unsafe { File::from_fd(fd) };
        Ok(file)
    }
}
impl Drop for OwnedFile {
    fn drop(&mut self) {
        unsafe { close(self.fd) }
    }
}

