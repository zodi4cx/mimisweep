use anyhow::Result;

fn main() -> Result<()> {
    pretty_env_logger::init();
    minesweeper::info()?;
    Ok(())
}