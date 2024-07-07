mod error;

use std::{io::Read, thread};

use binder::{binder_impl::IBinderInternal, get_service, SpIBinder};

/// The main entry of this crate
#[cfg(target_os = "android")]
pub struct Dumpsys {
    service: SpIBinder,
}

#[cfg(target_os = "android")]
impl Dumpsys {
    /// Retrieve an existing service and save it for dump, blocking for a few seconds if it doesn't yet exist.
    ///
    /// For example
    ///
    /// ```sh
    /// dumpsys SurfaceFlinger
    /// ```
    ///
    /// is equal to
    ///
    /// ```
    /// use dumpsys_rs::Dumpsys;
    ///
    /// Dumpsys::new("SurfaceFlinger");
    /// ```
    pub fn new<S>(service_name: S) -> Option<Self>
    where
        S: AsRef<str>,
    {
        let service = get_service(service_name.as_ref())?;
        Some(Self { service })
    }

    /// # Example
    ///
    /// ```
    /// use dumpsys_rs::Dumpsys;
    ///
    /// # fn foo() -> Option<()> {
    /// let result = Dumpsys::new("SurfaceFlinger")?
    ///     .dump(&["--latency"])
    ///     .unwrap();
    /// println!("{result}");
    /// # Some(())
    /// # }
    /// ```
    pub fn dump(&self, args: &'static [&str]) -> Result<String, error::DumpError> {
        let mut buf = String::new();

        {
            let mut service = self.service.clone();
            let (mut read, write) = os_pipe::pipe()?;
            let handle = thread::spawn(move || service.dump(&write, args));
            let _ = read.read_to_string(&mut buf);
            handle.join().unwrap()?;
        }

        Ok(buf)
    }
}
