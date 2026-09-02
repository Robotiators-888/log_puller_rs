// use std::io::Read;

use libssh_rs::Session;

fn main() -> anyhow::Result<()> {
    let session = Session::new()?;
    // session.set_option(libssh_rs::SshOption::Hostname(String::from("localhost")))?;
    session.set_option(libssh_rs::SshOption::Hostname(String::from("test.rebex.net")))?;
    session.connect()?;
    // session.userauth_keyboard_interactive(None, None)?;
    session.userauth_password(Some("demo"), Some("password"))?;
    let sftp = session.sftp()?;
    let mut rfile = sftp.open("readme.txt", libssh_rs::OpenFlags::READ_ONLY, 0)?;
    let mut file = std::fs::File::create("readme.txt")?;
    std::io::copy(&mut rfile, &mut file)?;
    anyhow::Ok(())
}
