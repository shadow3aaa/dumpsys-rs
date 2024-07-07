# dumpsys-rs

`dumpsys-rs` is a Rust library for retrieving and dumping service information in an Android system. It provides a convenient way to fetch detailed status information from different system services, similar to the `dumpsys` command in the Android shell.

## Features

- Retrieve handles for Android system services.
- Perform dump operations on services to obtain detailed system state information.

## Usage

1. Initialize the `Dumpsys` struct with the desired service name.
2. Use the `dump` method with a list of arguments to get the service dump information.

## Example

```rust
use dumpsys_rs::{Dumpsys, error::Error};

/* equals to dumpsys SurfaceFlinger --latency */
let result: Result<String, Error> = Dumpsys::new("SurfaceFlinger")
    .unwrap()
    .dump(&["--latency"]);
```

## License

`dumpsys-rs` is licensed under [`GNU General Public License v3.0 only`](LICENSE).
