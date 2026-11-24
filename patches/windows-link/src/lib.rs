//! Local replacement for `windows-link` 0.2.1.
//!
//! The upstream crate always uses `raw-dylib`, which on the `x86_64-pc-windows-gnu`
//! toolchain needs binutils `dlltool` plus an assembler. This machine has neither,
//! so on that target we link against the import library that ships with
//! `windows_x86_64_gnu` instead (the same approach `windows-targets` 0.52/0.53 use).
//! Every other target keeps the upstream behaviour.
#![no_std]

#[cfg(all(target_arch = "x86_64", target_env = "gnu", not(target_abi = "llvm")))]
#[doc(hidden)]
pub use windows_x86_64_gnu as _import_lib;

/// Defines an external function to import.
#[cfg(all(target_arch = "x86_64", target_env = "gnu", not(target_abi = "llvm")))]
#[macro_export]
macro_rules! link {
    ($library:literal $abi:literal $($link_name:literal)? fn $($function:tt)*) => (
        #[link(name = "windows.0.53.0")]
        extern $abi {
            $(#[link_name=$link_name])?
            pub fn $($function)*;
        }
    )
}

/// Defines an external function to import.
#[cfg(all(windows, target_arch = "x86"))]
#[macro_export]
macro_rules! link {
    ($library:literal $abi:literal $($link_name:literal)? fn $($function:tt)*) => (
        #[link(name = $library, kind = "raw-dylib", modifiers = "+verbatim", import_name_type = "undecorated")]
        extern $abi {
            $(#[link_name=$link_name])?
            pub fn $($function)*;
        }
    )
}

/// Defines an external function to import.
#[cfg(all(windows, not(target_arch = "x86"), not(all(target_arch = "x86_64", target_env = "gnu", not(target_abi = "llvm")))))]
#[macro_export]
macro_rules! link {
    ($library:literal $abi:literal $($link_name:literal)? fn $($function:tt)*) => (
        #[link(name = $library, kind = "raw-dylib", modifiers = "+verbatim")]
        extern $abi {
            $(#[link_name=$link_name])?
            pub fn $($function)*;
        }
    )
}

/// Defines an external function to import.
#[cfg(not(windows))]
#[macro_export]
macro_rules! link {
    ($library:literal $abi:literal $($link_name:literal)? fn $($function:tt)*) => (
        extern $abi {
            pub fn $($function)*;
        }
    )
}
