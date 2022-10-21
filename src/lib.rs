//! # `antlion`
//!
//! A magical _meta_ function that evaluate (at compile-time if used inside a
//! macro which is the point of taking a `TokenStream` input) any Rust expr!
//!
//! ```rust
//! use antlion::Sandbox;
//! use quote::quote;
//!
//! let test = Sandbox::new().unwrap();
//! let x: u32 = test.eval(quote! { 2 + 2 }).unwrap();
//! assert!(x == 4);
//! ```
//!
//! This library indeed is not what would benefit the most your crate build
//! time, but it was still design in mind with the will of caching sandbox
//! compilation.

use proc_macro2::TokenStream;
use quote::quote;
use std::io::Result;
use std::path::PathBuf;
use std::process::Command;
use std::str::FromStr;
use std::sync::Mutex;
use std::{env, fs, io};
use uuid::Uuid;

/// Internal representation of a `Sandbox`
///
/// A `Sandbox` is a throwable Cargo project made to evaluate arbitrary Rust
/// expression.
#[non_exhaustive]
pub struct Sandbox {
    lock: Mutex<()>,
    root_dir: PathBuf,
}

impl Sandbox {
    /// Create a `Sandbox` in `$OUT_DIR` folder
    ///
    /// `$OUT_DIR` is set by Cargo when `build.rs` is present :)
    pub fn new() -> Result<Self> {
        let out_dir = env!("OUT_DIR");
        let mut root_dir = PathBuf::from(out_dir);
        root_dir.push(Uuid::new_v4().to_string());
        Command::new("mkdir")
            .args(&["-p", root_dir.to_str().unwrap()])
            .output()?;
        Command::new("cargo")
            .current_dir(&root_dir)
            .args(&["new", "sandbox"])
            .output()?;
        root_dir.push("sandbox");
        Ok(Sandbox {
            root_dir,
            lock: Mutex::new(()),
        })
    }

    /// Rely on `cargo add` to install dependencies in your sandbox
    ///
    /// https://doc.rust-lang.org/cargo/commands/cargo-add.html
    pub fn deps(self, deps: &[&str]) -> Result<Self> {
        let Self { lock, root_dir } = &self;
        let _ = lock.lock().unwrap();
        for dep in deps {
            Command::new("cargo")
                .args(&["add", dep])
                .current_dir(&root_dir)
                .output()?;
        }
        Ok(self)
    }

    /// Evaluate in the Sandbox a given Rust expression
    ///
    /// `quote! { }` would help you to generate a `proc_macro2::TokenStream`
    pub fn eval<T: FromStr + ToString>(self, expr: TokenStream) -> Result<T> {
        let Self { lock, root_dir } = self;
        let _ = lock.lock().unwrap();
        let wrapper = quote! {
            use std::io::prelude::*;
            fn main() -> std::io::Result<()> {
                let mut file = std::fs::File::create("output")?;
                let output = { #expr }.to_string();
                file.write_all(output.as_bytes())?;
                Ok(())
            }
        };
        fs::write(root_dir.join("src/main.rs"), wrapper.to_string())?;
        Command::new("cargo")
            .arg("run")
            .current_dir(&root_dir)
            .output()?;
        let output = fs::read_to_string(root_dir.join("output"))?
            .parse()
            .or(Err(io::ErrorKind::Other))?;
        fs::remove_file(root_dir.join("output"))?;
        Ok(output)
    }
}