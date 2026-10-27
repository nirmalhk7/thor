#![allow(deprecated)]
#![allow(unexpected_cfgs)]

use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[cfg(feature = "cef-runtime")]
fn main() -> anyhow::Result<()> {
    thor::browser::cef::run()
}

#[cfg(not(feature = "cef-runtime"))]
fn main() -> anyhow::Result<()> {
    anyhow::bail!("Thor requires the embedded CEF runtime; rebuild with the default features")
}
