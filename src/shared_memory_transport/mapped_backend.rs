use std::ffi::CString;
use std::fs::{self, OpenOptions};
use std::hash::{Hash, Hasher};
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::path::{Path, PathBuf};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::error::CameraManError;
use crate::media_time::{monotonic_time_nanos, new_generation_id};

use super::protocol::{
    INVALID_SLOT, MAGIC, SLOT_COUNT, SharedHeader, aligned_header_len, mapped_len,
};
use super::slot_state_machine::reset_for_new_writer;
use super::validation::{SharedHeaderMetadata, validate_shared_header_metadata};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SharedFrameEndpoint {
    Posix(String),
    File(PathBuf),
    Unavailable(&'static str),
}

impl SharedFrameEndpoint {
    pub fn description(&self) -> String {
        match self {
            Self::Posix(name) => format!("POSIX {name}"),
            Self::File(path) => format!("App Group mmap {}", path.display()),
            Self::Unavailable(reason) => format!("unavailable: {reason}"),
        }
    }

    fn create_or_open_writer(&self) -> Result<(OwnedFd, bool), CameraManError> {
        match self {
            Self::Posix(name) => {
                let name = shared_memory_name(name)?;
                create_or_open_shared_memory(&name).map_err(Into::into)
            }
            Self::File(path) => create_or_open_file(path).map_err(Into::into),
            Self::Unavailable(reason) => Err(CameraManError::VirtualCameraUnavailable(reason)),
        }
    }

    fn open_reader(&self) -> Result<Option<OwnedFd>, CameraManError> {
        let result = match self {
            Self::Posix(name) => {
                let name = shared_memory_name(name)?;
                open_shared_memory(&name, libc::O_RDWR)
            }
            Self::File(path) => open_file(path),
            Self::Unavailable(reason) => {
                return Err(CameraManError::VirtualCameraUnavailable(reason));
            }
        };
        match result {
            Ok(fd) => Ok(Some(fd)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    pub(super) fn cleanup(&self) {
        match self {
            Self::Posix(name) => {
                if let Ok(name) = shared_memory_name(name) {
                    // SAFETY: `name` is a live NUL-terminated POSIX shm name;
                    // unlink does not retain the pointer.
                    unsafe {
                        libc::shm_unlink(name.as_ptr());
                    }
                }
            }
            Self::File(path) => {
                let _ = fs::remove_file(path);
            }
            Self::Unavailable(_) => {}
        }
    }
}

pub fn default_shared_memory_name() -> String {
    std::env::var("CAMERAMAN_SHARED_MEMORY_NAME").unwrap_or_else(|_| {
        // SAFETY: `geteuid` takes no arguments and has no memory contract.
        let user_id = unsafe { libc::geteuid() };
        format!("/cameraman-frame-v5-{user_id}")
    })
}

pub fn default_shared_frame_endpoint() -> SharedFrameEndpoint {
    match crate::app_group::shared_frame_file_path() {
        Ok(Some(path)) => SharedFrameEndpoint::File(path),
        Ok(None) => SharedFrameEndpoint::Posix(default_shared_memory_name()),
        Err(_) => SharedFrameEndpoint::Unavailable("App Group frame container is unavailable"),
    }
}

pub(super) struct MappedRegion {
    fd: OwnedFd,
    writer_lock: Option<OwnedFd>,
    ptr: NonNull<u8>,
    len: usize,
    writer_identity: Option<WriterIdentity>,
}

#[derive(Clone, Copy)]
struct WriterIdentity {
    pid: u32,
    generation: u64,
}

// SAFETY: moving the mapping transfers its owning fd and pointer together.
// It is not `Sync`; slot atomics and the reader lease serialize all byte
// access when a moved region is subsequently used by another thread.
unsafe impl Send for MappedRegion {}

impl MappedRegion {
    pub(super) fn create_writer(
        endpoint: &SharedFrameEndpoint,
        slot_capacity: usize,
    ) -> Result<Self, CameraManError> {
        let (fd, created) = endpoint.create_or_open_writer()?;
        let len = mapped_len(slot_capacity)?;
        let current_len = file_len(fd.as_raw_fd())?;
        if created {
            let length = libc::off_t::try_from(len)
                .map_err(|_| CameraManError::BufferTooLarge { bytes: len as u64 })?;
            // SAFETY: the owned descriptor is open for writing and `length`
            // was checked to fit `off_t`.
            if unsafe { libc::ftruncate(fd.as_raw_fd(), length) } != 0 {
                return Err(io::Error::last_os_error().into());
            }
        } else if current_len != len {
            return Err(CameraManError::VirtualCameraUnavailable(
                "existing shared-memory object has an incompatible size",
            ));
        }

        let mut region = Self::map(fd, len)?;
        region.writer_lock = open_writer_lock(endpoint)?;
        if created {
            region.initialize(slot_capacity);
        } else if !region.header_is_valid(slot_capacity) {
            return Err(CameraManError::VirtualCameraUnavailable(
                "existing shared-memory object has an incompatible header",
            ));
        }
        region.claim_writer()?;
        region.reset_for_new_writer();
        Ok(region)
    }

    pub(super) fn try_open_reader(
        endpoint: &SharedFrameEndpoint,
    ) -> Result<Option<Self>, CameraManError> {
        let Some(fd) = endpoint.open_reader()? else {
            return Ok(None);
        };
        let len = file_len(fd.as_raw_fd())?;
        if len < aligned_header_len() {
            return Ok(None);
        }
        let region = Self::map(fd, len)?;
        if !region.header_is_self_consistent() {
            return Ok(None);
        }
        Ok(Some(region))
    }

    fn map(fd: OwnedFd, len: usize) -> Result<Self, CameraManError> {
        // SAFETY: `fd` remains owned by the resulting region, `len` is the
        // checked file size, and the mapping is released exactly once in Drop.
        let mapped = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                len,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd.as_raw_fd(),
                0,
            )
        };
        if mapped == libc::MAP_FAILED {
            return Err(io::Error::last_os_error().into());
        }
        let ptr = NonNull::new(mapped.cast::<u8>()).ok_or(
            CameraManError::VirtualCameraUnavailable("shared-memory mapping returned null"),
        )?;
        Ok(Self {
            fd,
            writer_lock: None,
            ptr,
            len,
            writer_identity: None,
        })
    }

    pub(super) fn header(&self) -> &SharedHeader {
        // SAFETY: mmap returns page-aligned storage, every constructor checks
        // that `len` covers the aligned header, and the mapping outlives `self`.
        unsafe { &*self.ptr.as_ptr().cast::<SharedHeader>() }
    }

    fn initialize(&self, slot_capacity: usize) {
        let header_ptr = self.ptr.as_ptr().cast::<SharedHeader>();
        // SAFETY: a newly created writer exclusively owns a suitably aligned
        // mapping whose first bytes are large enough for `SharedHeader`.
        unsafe {
            std::ptr::write(header_ptr, SharedHeader::new(slot_capacity));
        }
        self.header().magic.store(MAGIC, Ordering::Release);
    }

    fn header_is_valid(&self, slot_capacity: usize) -> bool {
        self.header_is_self_consistent()
            && self.header().slot_capacity.load(Ordering::Acquire) == slot_capacity as u64
    }

    fn header_is_self_consistent(&self) -> bool {
        // SAFETY: the mapping covers an aligned `AtomicU32` at offset zero;
        // every bit pattern is valid before the creator publishes MAGIC.
        let magic = unsafe { &*self.ptr.as_ptr().cast::<AtomicU32>() };
        let published_magic = magic.load(Ordering::Acquire);
        if published_magic != MAGIC {
            return false;
        }
        let header = self.header();
        validate_shared_header_metadata(SharedHeaderMetadata {
            magic: header.magic.load(Ordering::Acquire),
            version: header.version.load(Ordering::Acquire),
            slot_count: header.slot_count.load(Ordering::Acquire),
            active_slot: header.active_slot.load(Ordering::Acquire),
            slot_capacity: header.slot_capacity.load(Ordering::Acquire),
            mapping_len: self.len,
            page_size: super::protocol::page_size(),
        })
        .is_ok()
    }

    fn reset_for_new_writer(&self) {
        let header = self.header();
        header.active_slot.store(INVALID_SLOT, Ordering::Release);
        header.next_sequence.store(0, Ordering::Release);
        header.consumer_pid.store(0, Ordering::Release);
        header.consumer_generation.store(0, Ordering::Relaxed);
        header.consumer_sequence.store(0, Ordering::Relaxed);
        header
            .consumer_heartbeat_monotonic_nanos
            .store(0, Ordering::Relaxed);
        for slot in &header.slots {
            reset_for_new_writer(&slot.state, process_is_alive);
        }
    }

    fn claim_writer(&mut self) -> Result<(), CameraManError> {
        let current_pid = std::process::id();
        let lock_fd = self.writer_lock.as_ref().unwrap_or(&self.fd).as_raw_fd();
        // SAFETY: `lock_fd` belongs to `self` and stays open for the lock's
        // lifetime; flock does not retain any Rust pointer.
        if unsafe { libc::flock(lock_fd, libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(CameraManError::VirtualCameraUnavailable(
                "another shared-memory writer is already connected",
            ));
        }

        let process_start_token = process_start_token(current_pid).ok_or(
            CameraManError::VirtualCameraUnavailable("cannot read the writer process start time"),
        )?;
        let generation = new_generation_id();
        let started = monotonic_time_nanos();
        let header = self.header();
        header
            .writer_generation
            .store(generation, Ordering::Relaxed);
        header
            .writer_process_start_token
            .store(process_start_token, Ordering::Relaxed);
        header
            .writer_heartbeat_monotonic_nanos
            .store(started, Ordering::Relaxed);
        header.writer_pid.store(current_pid, Ordering::Release);
        self.writer_identity = Some(WriterIdentity {
            pid: current_pid,
            generation,
        });
        Ok(())
    }

    pub(super) fn slot_data_ptr(&self, slot: usize, slot_capacity: usize) -> *mut u8 {
        debug_assert!(slot < SLOT_COUNT);
        debug_assert_eq!(self.len, mapped_len(slot_capacity).unwrap());
        // SAFETY: the checked slot offset is within the mapping established
        // for this exact capacity. Callers separately bound their byte count.
        unsafe {
            self.ptr.as_ptr().add(
                super::protocol::checked_slot_offset(slot, slot_capacity)
                    .expect("validated slot offset"),
            )
        }
    }
}

impl Drop for MappedRegion {
    fn drop(&mut self) {
        if let Some(identity) = self.writer_identity {
            let header = self.header();
            if header.writer_generation.load(Ordering::Acquire) == identity.generation {
                let cleared = header.writer_pid.compare_exchange(
                    identity.pid,
                    0,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                );
                if cleared.is_ok() {
                    header
                        .writer_process_start_token
                        .store(0, Ordering::Relaxed);
                    header
                        .writer_heartbeat_monotonic_nanos
                        .store(0, Ordering::Relaxed);
                    header.writer_generation.store(0, Ordering::Release);
                }
            }
            // SAFETY: the lock descriptor is still owned by `self`; releasing
            // an advisory lock does not access Rust memory.
            unsafe {
                let lock_fd = self.writer_lock.as_ref().unwrap_or(&self.fd).as_raw_fd();
                libc::flock(lock_fd, libc::LOCK_UN);
            }
        }
        // SAFETY: `ptr` and `len` are the exact live pair returned by mmap and
        // this Drop path runs once after all borrows of `self` have ended.
        unsafe {
            libc::munmap(self.ptr.as_ptr().cast(), self.len);
        }
    }
}

fn shared_memory_name(name: &str) -> Result<CString, CameraManError> {
    if !name.starts_with('/') || name[1..].contains('/') {
        return Err(CameraManError::VirtualCameraUnavailable(
            "shared-memory name must start with one slash and contain no other slash",
        ));
    }
    CString::new(name).map_err(|_| {
        CameraManError::VirtualCameraUnavailable("shared-memory name contains a null byte")
    })
}

fn open_shared_memory(name: &CString, flags: i32) -> io::Result<OwnedFd> {
    // SAFETY: `name` is NUL-terminated for the call and `flags`/mode are valid
    // POSIX values; shm_open does not retain the string pointer.
    let fd = unsafe { libc::shm_open(name.as_ptr(), flags, 0o600) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the non-negative descriptor was just returned with unique
    // ownership by shm_open and is transferred exactly once to `OwnedFd`.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

fn create_or_open_shared_memory(name: &CString) -> io::Result<(OwnedFd, bool)> {
    match open_shared_memory(name, libc::O_CREAT | libc::O_EXCL | libc::O_RDWR) {
        Ok(fd) => Ok((fd, true)),
        Err(error) if error.raw_os_error() == Some(libc::EEXIST) => {
            open_shared_memory(name, libc::O_RDWR).map(|fd| (fd, false))
        }
        Err(error) => Err(error),
    }
}

fn open_file(path: &Path) -> io::Result<OwnedFd> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map(Into::into)
}

fn create_or_open_file(path: &Path) -> io::Result<(OwnedFd, bool)> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    match OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(file) => Ok((file.into(), true)),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            open_file(path).map(|file| (file, false))
        }
        Err(error) => Err(error),
    }
}

fn open_writer_lock(endpoint: &SharedFrameEndpoint) -> Result<Option<OwnedFd>, CameraManError> {
    let SharedFrameEndpoint::Posix(name) = endpoint else {
        return Ok(None);
    };

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    name.hash(&mut hasher);
    // SAFETY: `geteuid` takes no arguments and has no memory contract.
    let user_id = unsafe { libc::geteuid() };
    let path = std::env::temp_dir().join(format!(
        "cameraman-writer-{}-{:016x}.lock",
        user_id,
        hasher.finish()
    ));
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    Ok(Some(file.into()))
}

fn file_len(fd: i32) -> io::Result<usize> {
    let mut status = std::mem::MaybeUninit::<libc::stat>::uninit();
    // SAFETY: `status` points to writable storage for one `stat`; `fd` is a
    // live descriptor borrowed by the caller for the duration of this call.
    if unsafe { libc::fstat(fd, status.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: a successful fstat initializes the complete output structure.
    let status = unsafe { status.assume_init() };
    usize::try_from(status.st_size)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "negative shared-memory size"))
}

pub(super) fn process_is_alive(pid: u32) -> bool {
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return false;
    };
    // SAFETY: signal zero performs existence/permission checking only; the
    // converted pid is passed by value and no Rust memory is accessed.
    if unsafe { libc::kill(pid, 0) } == 0 {
        return true;
    }
    io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(target_os = "macos")]
pub(super) fn process_start_token(pid: u32) -> Option<u64> {
    if pid == 0 {
        return None;
    }
    let pid = libc::pid_t::try_from(pid).ok()?;
    let mut info = std::mem::MaybeUninit::<libc::proc_bsdinfo>::zeroed();
    let expected = std::mem::size_of::<libc::proc_bsdinfo>();
    let buffer_size = libc::c_int::try_from(expected).ok()?;
    // SAFETY: `info` is writable for exactly `buffer_size` bytes and the
    // requested PROC_PIDTBSDINFO flavor produces `proc_bsdinfo`.
    let bytes_read = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTBSDINFO,
            0,
            info.as_mut_ptr().cast(),
            buffer_size,
        )
    };
    if usize::try_from(bytes_read).ok()? != expected {
        return None;
    }
    // SAFETY: the exact expected byte count confirms complete initialization.
    let info = unsafe { info.assume_init() };
    info.pbi_start_tvsec
        .checked_mul(1_000_000)
        .and_then(|seconds| seconds.checked_add(info.pbi_start_tvusec))
        .filter(|token| *token != 0)
}

#[cfg(not(target_os = "macos"))]
pub(super) fn process_start_token(pid: u32) -> Option<u64> {
    (pid == std::process::id() && process_is_alive(pid))
        .then(monotonic_time_nanos)
        .filter(|token| *token != 0)
}

#[cfg(target_os = "macos")]
pub(super) fn process_matches_start_token(pid: u32, expected: u64) -> bool {
    expected != 0 && process_start_token(pid) == Some(expected)
}

#[cfg(not(target_os = "macos"))]
pub(super) fn process_matches_start_token(pid: u32, expected: u64) -> bool {
    expected != 0 && process_is_alive(pid)
}
