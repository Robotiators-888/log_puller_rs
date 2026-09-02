use libssh_rs::Session;

fn main() -> anyhow::Result<()> {
    println!("Hello, world!");
    let session = Session::new()?.sftp()?;
    anyhow::Ok(())
}
