pub mod error;
mod task_thread;

use std::{
    self,
    collections::HashMap,
    hash::BuildHasherDefault,
    io::{self, PipeWriter, Read as _},
    ops::Deref,
    sync::{
        Arc,
        atomic::{AtomicI32, Ordering},
    },
};

use rsbinder::{ProcessState, SIBinder, StatusCode, hub};
use twox_hash::XxHash3_64;

use crate::{error::Error, task_thread::TaskThread};

type Result<T, E = crate::error::Error> = core::result::Result<T, E>;

/// One shot dumpsys
///
/// # Example
///
/// ```sh
/// dumpsys SurfaceFlinger
/// ```
///
/// is equal to
///
/// ```no_run
/// # fn foo() -> Result<(), dumpsys_rs::error::Error> {
/// dumpsys_rs::dump("SurfaceFlinger", &[])?;
/// # Ok(())
/// # }
/// ```
pub fn dump<S: AsRef<str>>(service_name: S, args: &[&str]) -> Result<String> {
    _ = ProcessState::init_default();

    let task_thread = TaskThread::spawn();

    let service = hub::get_service(service_name.as_ref()).ok_or(Error::ServiceNotExist)?;

    dump_inner(&task_thread, service, args)
}

/// Dumps a service into a fixed-size byte array.
///
/// # Example
///
/// ```no_run
/// # fn foo() -> Result<(), dumpsys_rs::error::Error> {
/// let bytes: [u8; 1024] = dumpsys_rs::dump_to_byte("SurfaceFlinger", &["--latency"])?;
/// # Ok(())
/// # }
/// ```
pub fn dump_to_byte<S: AsRef<str>, const N: usize>(
    service_name: S,
    args: &[&str],
) -> Result<[u8; N]> {
    _ = ProcessState::init_default();

    let task_thread = TaskThread::spawn();

    let service = hub::get_service(service_name.as_ref()).ok_or(Error::ServiceNotExist)?;

    dump_to_byte_inner(&task_thread, service, args)
}

#[repr(transparent)]
struct DumpArgs {
    inner: Box<[String]>,
}

impl FromIterator<String> for DumpArgs {
    fn from_iter<T: IntoIterator<Item = String>>(iter: T) -> Self {
        let inner: Box<[String]> = iter.into_iter().collect();
        Self { inner }
    }
}

impl Deref for DumpArgs {
    type Target = [String];

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

type StatusI32Slot = Arc<AtomicI32>;

enum Task {
    Dump(DumpArgs, PipeWriter, SIBinder, StatusI32Slot),
    Shutdown,
}

/// Single retrieved existing services.
///
/// Like [`Dumpsys`], but use a task_thread exclusively.
pub struct BoundDumpsys {
    service: SIBinder,
    task_thread: TaskThread,
}

impl BoundDumpsys {
    /// Retrieve an existing service and save it for dump, blocking for a few seconds if it doesn't yet exist.
    ///
    /// # Example
    ///
    /// ```sh
    /// dumpsys SurfaceFlinger
    /// ```
    ///
    /// is equal to
    ///
    /// ```no_run
    /// use dumpsys_rs::BoundDumpsys;
    ///
    /// # fn foo() -> Result<(), dumpsys_rs::error::Error> {
    /// let mut dumpsys = BoundDumpsys::new("SurfaceFlinger")?;
    /// let result = dumpsys
    ///     .dump(&[])?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn new<S: AsRef<str>>(service_name: S) -> Result<Self> {
        _ = ProcessState::init_default();

        Ok(Self {
            service: hub::get_service(service_name.as_ref()).ok_or(Error::ServiceNotExist)?,
            task_thread: TaskThread::spawn(),
        })
    }

    pub fn dump(&self, args: &[&str]) -> Result<String> {
        dump_inner(&self.task_thread, self.service.clone(), args)
    }

    /// Dumps the bound service into a fixed-size byte array.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use dumpsys_rs::BoundDumpsys;
    ///
    /// # fn foo() -> Result<(), dumpsys_rs::error::Error> {
    /// let bytes: [u8; 1024] = BoundDumpsys::new("SurfaceFlinger")?
    ///     .dump_to_byte(&["--latency"])?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn dump_to_byte<const N: usize>(&self, args: &[&str]) -> Result<[u8; N]> {
        dump_to_byte_inner(&self.task_thread, self.service.clone(), args)
    }
}

type XxHashMap<K, V> = HashMap<K, V, BuildHasherDefault<XxHash3_64>>;

/// Retrieved existing services.
///
/// Drop [`Dumpsys`] will exit the background pipeing thread
pub struct Dumpsys {
    map: XxHashMap<Box<str>, SIBinder>,
    task_thread: TaskThread,
}

impl Dumpsys {
    pub fn new() -> Result<Self> {
        _ = ProcessState::init_default();

        Ok(Self {
            map: XxHashMap::default(),
            task_thread: TaskThread::spawn(),
        })
    }

    /// Retrieve an existing service and save it for dump, blocking for a few seconds if it doesn't yet exist.
    ///
    /// # Example
    ///
    /// ```sh
    /// dumpsys SurfaceFlinger
    /// ```
    ///
    /// is equal to
    ///
    /// ```no_run
    /// use dumpsys_rs::Dumpsys;
    ///
    /// # fn foo() -> Result<(), dumpsys_rs::error::Error> {
    /// let mut dumpsys = Dumpsys::new()?;
    /// dumpsys.insert_service("SurfaceFlinger")?;
    /// let result = dumpsys
    ///     .dump("SurfaceFlinger", &[])?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn insert_service<S: AsRef<str>>(&mut self, service_name: S) -> Result<bool> {
        let service_name = service_name.as_ref();

        let service = hub::get_service(service_name).ok_or(Error::ServiceNotExist)?;

        Ok(self.map.insert(Box::from(service_name), service).is_some())
    }

    /// Removes the selected service from the inner [`HashMap`].
    pub fn remove_service<S: AsRef<str>>(&mut self, service_name: S) -> Result<SIBinder> {
        let service_name = service_name.as_ref();

        self.map.remove(service_name).ok_or(Error::NoEntryFound)
    }

    pub fn dump<S: AsRef<str>>(&mut self, service_name: S, args: &[&str]) -> Result<String> {
        let service_name = service_name.as_ref();

        let service = self.map.get(service_name).ok_or(Error::NoEntryFound)?;

        dump_inner(&self.task_thread, service.clone(), args)
    }

    /// Dumps the selected service into a fixed-size byte array.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use dumpsys_rs::Dumpsys;
    ///
    /// # fn foo() -> Result<(), dumpsys_rs::error::Error> {
    /// let mut dumpsys = Dumpsys::new()?;
    /// dumpsys.insert_service("SurfaceFlinger")?;
    /// let bytes: [u8; 1024] = dumpsys
    ///     .dump_to_byte("SurfaceFlinger", &["--latency"])?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn dump_to_byte<S: AsRef<str>, const N: usize>(
        &mut self,
        service_name: S,
        args: &[&str],
    ) -> Result<[u8; N]> {
        let service_name = service_name.as_ref();

        let service = self.map.get(service_name).ok_or(Error::NoEntryFound)?;

        dump_to_byte_inner(&self.task_thread, service.clone(), args)
    }
}

fn dump_inner(task_thread: &TaskThread, service: SIBinder, args: &[&str]) -> Result<String> {
    let (mut reader, writer) = io::pipe()?;

    let status_i32 = Arc::new(AtomicI32::new(i32::from(StatusCode::Ok)));

    task_thread
        .send(Task::Dump(
            DumpArgs::from_iter(args.iter().copied().map(String::from)),
            writer,
            service,
            status_i32.clone(),
        ))
        .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "task_thread dropped receiver"))?;

    let mut buf = String::new();
    reader.read_to_string(&mut buf)?;

    let status_code = StatusCode::from(status_i32.load(Ordering::Relaxed));
    if !matches!(status_code, StatusCode::Ok) {
        Err(status_code)?;
    }

    Ok(buf)
}

fn dump_to_byte_inner<const N: usize>(
    task_thread: &TaskThread,
    service: SIBinder,
    args: &[&str],
) -> Result<[u8; N]> {
    let (mut reader, writer) = io::pipe()?;

    let status_i32 = Arc::new(AtomicI32::new(i32::from(StatusCode::Ok)));

    task_thread
        .send(Task::Dump(
            DumpArgs::from_iter(args.iter().copied().map(String::from)),
            writer,
            service,
            status_i32.clone(),
        ))
        .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "task_thread dropped receiver"))?;

    let mut buf = [0u8; N];
    let _ = reader.read_exact(&mut buf);

    let status_code = StatusCode::from(status_i32.load(Ordering::Relaxed));
    if !matches!(status_code, StatusCode::Ok) {
        Err(status_code)?;
    }

    Ok(buf)
}
