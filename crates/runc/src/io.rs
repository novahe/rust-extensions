/*
   Copyright The containerd Authors.

   Licensed under the Apache License, Version 2.0 (the "License");
   you may not use this file except in compliance with the License.
   You may obtain a copy of the License at

       http://www.apache.org/licenses/LICENSE-2.0

   Unless required by applicable law or agreed to in writing, software
   distributed under the License is distributed on an "AS IS" BASIS,
   WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
   See the License for the specific language governing permissions and
   limitations under the License.
*/
#[cfg(not(feature = "async"))]
use std::io::{Read, Write};
use std::{
    fmt::Debug,
    fs::{File, OpenOptions},
    io::Result,
    os::unix::{
        fs::OpenOptionsExt,
        io::{AsRawFd, FromRawFd, IntoRawFd, OwnedFd, RawFd},
    },
    process::Stdio,
    sync::Mutex,
};

use nix::unistd::{Gid, Uid};
#[cfg(not(feature = "async"))]
use os_pipe::{PipeReader, PipeWriter};
#[cfg(feature = "async")]
use tokio::io::{AsyncRead, AsyncWrite};

use crate::Command;

pub trait Io: Debug + Send + Sync {
    /// Return write side of stdin
    #[cfg(not(feature = "async"))]
    fn stdin(&self) -> Option<Box<dyn Write + Send + Sync>> {
        None
    }

    /// Return read side of stdout
    #[cfg(not(feature = "async"))]
    fn stdout(&self) -> Option<Box<dyn Read + Send>> {
        None
    }

    /// Return read side of stderr
    #[cfg(not(feature = "async"))]
    fn stderr(&self) -> Option<Box<dyn Read + Send>> {
        None
    }

    /// Return write side of stdin
    #[cfg(feature = "async")]
    fn stdin(&self) -> Option<Box<dyn AsyncWrite + Send + Sync + Unpin>> {
        None
    }

    /// Return read side of stdout
    #[cfg(feature = "async")]
    fn stdout(&self) -> Option<Box<dyn AsyncRead + Send + Sync + Unpin>> {
        None
    }

    /// Return read side of stderr
    #[cfg(feature = "async")]
    fn stderr(&self) -> Option<Box<dyn AsyncRead + Send + Sync + Unpin>> {
        None
    }

    /// Set IO for passed command.
    /// Read side of stdin, write side of stdout and write side of stderr should be provided to command.
    fn set(&self, cmd: &mut Command) -> Result<()>;

    /// Only close write side (should be stdout/err "from" runc process)
    fn close_after_start(&self);
}

#[derive(Debug, Clone)]
pub struct IOOption {
    pub open_stdin: bool,
    pub open_stdout: bool,
    pub open_stderr: bool,
}

impl Default for IOOption {
    fn default() -> Self {
        Self {
            open_stdin: true,
            open_stdout: true,
            open_stderr: true,
        }
    }
}

/// Struct to represent a pipe that can be used to transfer stdio inputs and outputs.
///
/// With this Io driver, methods of [crate::Runc] may capture the output/error messages.
/// Each fd is wrapped in `Mutex<Option<OwnedFd>>` so that ownership can be
/// transferred out via `take()`. Any fd still present when `Pipe` is dropped
/// will be closed automatically by `OwnedFd::drop`.
#[derive(Debug)]
pub struct Pipe {
    rd: Mutex<Option<OwnedFd>>,
    wr: Mutex<Option<OwnedFd>>,
}

#[derive(Debug)]
pub struct PipedIo {
    stdin: Option<Pipe>,
    stdout: Option<Pipe>,
    stderr: Option<Pipe>,
}

impl Pipe {
    fn new() -> std::io::Result<Self> {
        let (rd, wr) = os_pipe::pipe()?;
        Ok(Self {
            rd: Mutex::new(Some(unsafe { OwnedFd::from_raw_fd(rd.into_raw_fd()) })),
            wr: Mutex::new(Some(unsafe { OwnedFd::from_raw_fd(wr.into_raw_fd()) })),
        })
    }

    /// Take ownership of the read-end fd. Returns the raw fd and
    /// prevents this `Pipe` from closing it on drop.
    fn take_rd(&self) -> Option<RawFd> {
        self.rd.lock().unwrap().take().map(|fd| fd.into_raw_fd())
    }

    /// Take ownership of the write-end fd. Returns the raw fd and
    /// prevents this `Pipe` from closing it on drop.
    fn take_wr(&self) -> Option<RawFd> {
        self.wr.lock().unwrap().take().map(|fd| fd.into_raw_fd())
    }

    /// Get the raw fd of the read end without taking ownership.
    fn rd_raw(&self) -> Option<RawFd> {
        self.rd.lock().unwrap().as_ref().map(|fd| fd.as_raw_fd())
    }

    /// Get the raw fd of the write end without taking ownership.
    fn wr_raw(&self) -> Option<RawFd> {
        self.wr.lock().unwrap().as_ref().map(|fd| fd.as_raw_fd())
    }
}

impl PipedIo {
    pub fn new(uid: u32, gid: u32, opts: &IOOption) -> std::io::Result<Self> {
        Ok(Self {
            stdin: Self::create_pipe(uid, gid, opts.open_stdin, true)?,
            stdout: Self::create_pipe(uid, gid, opts.open_stdout, false)?,
            stderr: Self::create_pipe(uid, gid, opts.open_stderr, false)?,
        })
    }

    fn create_pipe(
        uid: u32,
        gid: u32,
        enabled: bool,
        stdin: bool,
    ) -> std::io::Result<Option<Pipe>> {
        if !enabled {
            return Ok(None);
        }

        let pipe = Pipe::new()?;
        let uid = Some(Uid::from_raw(uid));
        let gid = Some(Gid::from_raw(gid));
        if stdin {
            if let Some(raw) = pipe.rd_raw() {
                nix::unistd::fchown(raw, uid, gid)?;
            }
        } else {
            if let Some(raw) = pipe.wr_raw() {
                nix::unistd::fchown(raw, uid, gid)?;
            }
        }
        Ok(Some(pipe))
    }
}

impl Io for PipedIo {
    #[cfg(not(feature = "async"))]
    fn stdin(&self) -> Option<Box<dyn Write + Send + Sync>> {
        self.stdin.as_ref().and_then(|pipe| {
            pipe.take_wr().map(|fd| {
                let writer = unsafe { PipeWriter::from_raw_fd(fd) };
                Box::new(writer) as Box<dyn Write + Send + Sync>
            })
        })
    }

    #[cfg(feature = "async")]
    fn stdin(&self) -> Option<Box<dyn AsyncWrite + Send + Sync + Unpin>> {
        self.stdin.as_ref().and_then(|pipe| {
            pipe.take_wr().and_then(|fd| {
                tokio_pipe::PipeWrite::from_raw_fd_checked(fd)
                    .map(|x| Box::new(x) as Box<dyn AsyncWrite + Send + Sync + Unpin>)
                    .ok()
            })
        })
    }

    #[cfg(not(feature = "async"))]
    fn stdout(&self) -> Option<Box<dyn Read + Send>> {
        self.stdout.as_ref().and_then(|pipe| {
            pipe.take_rd().map(|fd| {
                let reader = unsafe { PipeReader::from_raw_fd(fd) };
                Box::new(reader) as Box<dyn Read + Send>
            })
        })
    }

    #[cfg(feature = "async")]
    fn stdout(&self) -> Option<Box<dyn AsyncRead + Send + Sync + Unpin>> {
        self.stdout.as_ref().and_then(|pipe| {
            pipe.take_rd().and_then(|fd| {
                tokio_pipe::PipeRead::from_raw_fd_checked(fd)
                    .map(|x| Box::new(x) as Box<dyn AsyncRead + Send + Sync + Unpin>)
                    .ok()
            })
        })
    }

    #[cfg(not(feature = "async"))]
    fn stderr(&self) -> Option<Box<dyn Read + Send>> {
        self.stderr.as_ref().and_then(|pipe| {
            pipe.take_rd().map(|fd| {
                let reader = unsafe { PipeReader::from_raw_fd(fd) };
                Box::new(reader) as Box<dyn Read + Send>
            })
        })
    }

    #[cfg(feature = "async")]
    fn stderr(&self) -> Option<Box<dyn AsyncRead + Send + Sync + Unpin>> {
        self.stderr.as_ref().and_then(|pipe| {
            pipe.take_rd().and_then(|fd| {
                tokio_pipe::PipeRead::from_raw_fd_checked(fd)
                    .map(|x| Box::new(x) as Box<dyn AsyncRead + Send + Sync + Unpin>)
                    .ok()
            })
        })
    }

    /// Transfer stdin.rd, stdout.wr, stderr.wr to the child command.
    /// The transferred fds are taken out of the `Pipe` so they won't be
    /// double-closed when `Pipe` is dropped.
    fn set(&self, cmd: &mut Command) -> std::io::Result<()> {
        if let Some(p) = self.stdin.as_ref() {
            if let Some(fd) = p.take_rd() {
                cmd.stdin(unsafe { Stdio::from_raw_fd(fd) });
            }
        }

        if let Some(p) = self.stdout.as_ref() {
            if let Some(fd) = p.take_wr() {
                cmd.stdout(unsafe { Stdio::from_raw_fd(fd) });
            }
        }

        if let Some(p) = self.stderr.as_ref() {
            if let Some(fd) = p.take_wr() {
                cmd.stderr(unsafe { Stdio::from_raw_fd(fd) });
            }
        }

        Ok(())
    }

    /// Close the write side of stdout/stderr pipes. If fds were already
    /// taken by `set()`, this is a no-op for those fds.
    fn close_after_start(&self) {
        if let Some(p) = self.stdout.as_ref() {
            // take() returns the OwnedFd which is then dropped → close
            drop(p.wr.lock().unwrap().take());
        }

        if let Some(p) = self.stderr.as_ref() {
            drop(p.wr.lock().unwrap().take());
        }
    }
}

/// IO driver to direct output/error messages to /dev/null.
///
/// With this Io driver, all methods of [crate::Runc] can't capture the output/error messages.
#[derive(Debug)]
pub struct NullIo {
    dev_null: Mutex<Option<File>>,
}

impl NullIo {
    pub fn new() -> std::io::Result<Self> {
        let f = OpenOptions::new().read(true).open("/dev/null")?;
        let dev_null = Mutex::new(Some(f));
        Ok(Self { dev_null })
    }
}

impl Io for NullIo {
    fn set(&self, cmd: &mut Command) -> std::io::Result<()> {
        if let Some(null) = self.dev_null.lock().unwrap().as_ref() {
            cmd.stdout(null.try_clone()?);
            cmd.stderr(null.try_clone()?);
        }
        Ok(())
    }

    fn close_after_start(&self) {
        let mut m = self.dev_null.lock().unwrap();
        let _ = m.take();
    }
}

/// Io driver based on Stdio::inherited(), to direct outputs/errors to stdio.
///
/// With this Io driver, all methods of [crate::Runc] can't capture the output/error messages.
#[derive(Debug)]
pub struct InheritedStdIo {}

impl InheritedStdIo {
    pub fn new() -> std::io::Result<Self> {
        Ok(InheritedStdIo {})
    }
}

impl Io for InheritedStdIo {
    fn set(&self, cmd: &mut Command) -> std::io::Result<()> {
        cmd.stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
        Ok(())
    }

    fn close_after_start(&self) {}
}

/// Io driver based on Stdio::piped(), to capture outputs/errors from runC.
///
/// With this Io driver, methods of [crate::Runc] may capture the output/error messages.
#[derive(Debug)]
pub struct PipedStdIo {}

impl PipedStdIo {
    pub fn new() -> std::io::Result<Self> {
        Ok(PipedStdIo {})
    }
}

impl Io for PipedStdIo {
    fn set(&self, cmd: &mut Command) -> std::io::Result<()> {
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        Ok(())
    }

    fn close_after_start(&self) {}
}

/// FIFO for the scenario that set FIFO for command Io.
#[derive(Debug)]
pub struct FIFO {
    pub stdin: Option<String>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
}

impl Io for FIFO {
    fn set(&self, cmd: &mut Command) -> Result<()> {
        if let Some(path) = self.stdin.as_ref() {
            let stdin = OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NONBLOCK)
                .open(path)?;
            cmd.stdin(stdin);
        }

        if let Some(path) = self.stdout.as_ref() {
            let stdout = OpenOptions::new().write(true).open(path)?;
            cmd.stdout(stdout);
        }

        if let Some(path) = self.stderr.as_ref() {
            let stderr = OpenOptions::new().write(true).open(path)?;
            cmd.stderr(stderr);
        }

        Ok(())
    }

    fn close_after_start(&self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn test_io_option() {
        let opts = IOOption {
            open_stdin: false,
            open_stdout: false,
            open_stderr: false,
        };
        let io = PipedIo::new(1000, 1000, &opts).unwrap();

        assert!(io.stdin().is_none());
        assert!(io.stdout().is_none());
        assert!(io.stderr().is_none());
    }

    #[cfg(target_os = "linux")]
    #[cfg(not(feature = "async"))]
    #[test]
    fn test_create_piped_io() {
        let opts = IOOption::default();
        let uid = nix::unistd::getuid();
        let gid = nix::unistd::getgid();
        let io = PipedIo::new(uid.as_raw(), gid.as_raw(), &opts).unwrap();
        let mut buf = [0xfau8];

        // stdin(): takes the write-end fd
        let mut stdin = io.stdin().unwrap();
        stdin.write_all(&buf).unwrap();
        buf[0] = 0x0;

        // read from stdin's read-end (still owned by Pipe)
        io.stdin.as_ref().map(|v| {
            let rd_fd = v.rd_raw().unwrap();
            let mut file = unsafe { File::from_raw_fd(rd_fd) };
            file.read(&mut buf).unwrap();
            // prevent File from closing the fd (still owned by Pipe)
            std::mem::forget(file);
        });
        assert_eq!(&buf, &[0xfau8]);

        // stdout(): takes the read-end fd
        let mut stdout = io.stdout().unwrap();
        buf[0] = 0xce;
        // write to stdout's write-end (still owned by Pipe)
        io.stdout.as_ref().map(|v| {
            let wr_fd = v.wr_raw().unwrap();
            let mut file = unsafe { File::from_raw_fd(wr_fd) };
            file.write(&buf).unwrap();
            std::mem::forget(file);
        });
        buf[0] = 0x0;
        stdout.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, &[0xceu8]);

        // stderr(): takes the read-end fd
        let mut stderr = io.stderr().unwrap();
        buf[0] = 0xa5;
        io.stderr.as_ref().map(|v| {
            let wr_fd = v.wr_raw().unwrap();
            let mut file = unsafe { File::from_raw_fd(wr_fd) };
            file.write(&buf).unwrap();
            std::mem::forget(file);
        });
        buf[0] = 0x0;
        stderr.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, &[0xa5u8]);

        // close_after_start drops stdout.wr and stderr.wr
        io.close_after_start();
        stdout.read_exact(&mut buf).unwrap_err();
        stderr.read_exact(&mut buf).unwrap_err();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_pipe_drop_closes_fds() {
        let pipe = Pipe::new().unwrap();
        let rd_fd = pipe.rd_raw().unwrap();
        let wr_fd = pipe.wr_raw().unwrap();

        // fds should be valid
        assert!(nix::fcntl::fcntl(rd_fd, nix::fcntl::FcntlArg::F_GETFD).is_ok());
        assert!(nix::fcntl::fcntl(wr_fd, nix::fcntl::FcntlArg::F_GETFD).is_ok());

        drop(pipe);

        // fds should be invalid after drop (EBADF)
        let rd_res = nix::fcntl::fcntl(rd_fd, nix::fcntl::FcntlArg::F_GETFD);
        let wr_res = nix::fcntl::fcntl(wr_fd, nix::fcntl::FcntlArg::F_GETFD);

        assert!(
            rd_res.is_err(),
            "rd_fd {} should be closed but fcntl returned {:?}",
            rd_fd,
            rd_res
        );
        assert!(
            wr_res.is_err(),
            "wr_fd {} should be closed but fcntl returned {:?}",
            wr_fd,
            wr_res
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_pipe_take_prevents_double_close() {
        let pipe = Pipe::new().unwrap();
        let rd_fd = pipe.take_rd().unwrap();
        let wr_fd = pipe.wr_raw().unwrap();

        // rd_fd should still be valid (not yet closed)
        assert!(unsafe { libc::fcntl(rd_fd, libc::F_GETFD) } >= 0);

        drop(pipe);

        // rd was taken, should still be valid; wr was not taken, should be closed
        assert!(unsafe { libc::fcntl(rd_fd, libc::F_GETFD) } >= 0);
        assert!(unsafe { libc::fcntl(wr_fd, libc::F_GETFD) } < 0);

        // clean up the taken fd
        nix::unistd::close(rd_fd).unwrap();
    }

    #[test]
    fn test_null_io() {
        let io = NullIo::new().unwrap();
        assert!(io.stdin().is_none());
        assert!(io.stdout().is_none());
        assert!(io.stderr().is_none());
    }
}
