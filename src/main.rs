use libssh_rs::Session;

fn main() -> anyhow::Result<()> {
    let session = Session::new()?;
    // session.set_option(libssh_rs::SshOption::Hostname(String::from("localhost")))?;
    session.set_option(libssh_rs::SshOption::Hostname(String::from("test.rebex.net")))?;
    session.connect()?;
    // session.userauth_keyboard_interactive(None, None)?;
    session.userauth_password(Some("demo"), Some("password"))?;
    let channel = session.new_channel()?;
    channel.open_session()?;
    println!("Session opened");
    channel.request_exec("whoami")?;
    channel.send_eof()?;
    anyhow::Ok(())
}
