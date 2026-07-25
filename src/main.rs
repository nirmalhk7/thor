use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

mod shell;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Launching Thor Native Shell...");
    shell::run().await?;
    Ok(())
}
