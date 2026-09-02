use std::io::Read;

use libssh_rs::Session;

fn main() -> anyhow::Result<()> {
    let session = Session::new()?;
    // session.set_option(libssh_rs::SshOption::Hostname(String::from("localhost")))?;
    session.set_option(libssh_rs::SshOption::Hostname(String::from("test.rebex.net")))?;
    session.connect()?;
    // session.userauth_keyboard_interactive(None, None)?;
    session.userauth_password(Some("demo"), Some("password"))?;
    let sftp = session.sftp()?;
    let mut dir = sftp.open("readme.txt", libssh_rs::OpenFlags::READ_ONLY, 0)?;
    let mut file_contents = String::new();
    dir.read_to_string(&mut file_contents)?;
    println!("File contents: {}", file_contents);
    let channel = session.new_channel()?;
    channel.open_session()?;
    // println!("Session opened");
    channel.request_exec("ls")?;
    channel.send_eof()?;
    let mut stdout = String::new();
    channel.stdout().read_to_string(&mut stdout)?;
    let res = channel.get_exit_status().unwrap();
    print!("stdout: {}", stdout);
    println!("res: {}", res);
    anyhow::Ok(())
}
