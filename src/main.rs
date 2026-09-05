// use std::io::Read;

use libssh_rs::Session;

fn main() -> anyhow::Result<()> {
    let session = Session::new()?;
    // session.set_option(libssh_rs::SshOption::Hostname(String::from("localhost")))?;
    session.set_option(libssh_rs::SshOption::Hostname(String::from(
        "test.rebex.net",
    )))?;
    session.connect()?;
    // session.userauth_keyboard_interactive(None, None)?;
    session.userauth_password(Some("demo"), Some("password"))?;
    let sftp = session.sftp()?;
    println!("Entries: {}", sftp.read_dir(".")?.into_iter().len());
    for (i, f) in sftp.read_dir(".")?.into_iter().enumerate() {
        if let libssh_rs::FileType::Directory = f
            .file_type()
            .ok_or(anyhow::anyhow!("Failed to get file type"))?
        {
            continue;
        }
        println!("Ran: {}", i);
        let name = f.name().ok_or(anyhow::anyhow!("Failed to get file name"))?;
        println!("Got name: {}", name);
        let mut rfile = sftp.open(name, libssh_rs::OpenFlags::READ_ONLY, 0)?;
        println!("Opened file");
        let mut file = std::fs::File::create(name)?;
        println!("Created file");
        std::io::copy(&mut rfile, &mut file)?;
        println!("Copied file");
    }
    anyhow::Ok(())
}
