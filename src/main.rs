use libssh_rs::{Session, Sftp};
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let session = Session::new()?;
    // session.set_option(libssh_rs::SshOption::Hostname(String::from("localhost")))?;
    session.set_option(libssh_rs::SshOption::Hostname(String::from(
        "test.rebex.net",
    )))?;
    session.connect()?;
    session.userauth_password(Some("demo"), Some("password"))?;
    let sftp = session.sftp()?;
    copy_folder(&sftp, Path::new("."), Path::new("data"))?;
    anyhow::Ok(())
}

fn copy_folder(sftp: &Sftp, dirpath: &Path, prefix: &Path) -> anyhow::Result<()> {
    println!("Path: {}", dirpath.to_str().unwrap());
    println!("Prefix: {}", prefix.to_str().unwrap());
    for f in sftp
        .read_dir(
            dirpath
                .to_str()
                .ok_or(anyhow::anyhow!("Couldn't convert path to str"))?,
        )?
        .into_iter()
    {
        let filetype = f
            .file_type()
            .ok_or(anyhow::anyhow!("Failed to get file type"))?;
        // If its a directory then call the function recursivley, copy if its a file, otherwise return an error
        if let libssh_rs::FileType::Directory = filetype {
            let dirname = f.name().ok_or(anyhow::anyhow!("Failed to get file name"))?;
            if dirname == "." || dirname == ".." {
                continue;
            }
            let prefixdir = &prefix.join(dirname);
            std::fs::create_dir_all(prefixdir)?;
            copy_folder(sftp, &dirpath.join(dirname), prefixdir)?;
        } else if let libssh_rs::FileType::Regular = filetype {
            let name = f.name().ok_or(anyhow::anyhow!("Failed to get file name"))?;
            println!("Name: {}", name);
            let mut rfile = sftp.open(
                &dirpath
                    .join(name)
                    .to_str()
                    .ok_or(anyhow::anyhow!("Failed to join remote file path"))?,
                libssh_rs::OpenFlags::READ_ONLY,
                0,
            )?;
            let mut file = std::fs::File::create(&prefix.join(name))?;
            std::io::copy(&mut rfile, &mut file)?;
        } else {
            return anyhow::Result::Err(anyhow::anyhow!("Invalid file type encountered"));
        }
    }
    anyhow::Ok(())
}
