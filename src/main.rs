use libssh_rs::{Session, Sftp};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

fn main() -> anyhow::Result<()> {
    // Is Box<Path> better?
    let mut map: HashMap<PathBuf, u64> = HashMap::new();
    // Use notifications to deal with this later
    populate_map(Path::new("data"), &mut map);
    println!("{:?}", map);
    // let mut map: Arc<Mutex<HashMap<PathBuf, u64>>> = Arc::new(Mutex::new(map));
    // // Is Box<Path> better again?
    // let mut filenames: Vec<(PathBuf, PathBuf)> = Vec::new();
    let session = Session::new()?;
    // session.set_option(libssh_rs::SshOption::Hostname(String::from("localhost")))?;
    session.set_option(libssh_rs::SshOption::Hostname(String::from(
        "test.rebex.net",
    )))?;
    session.connect()?;
    session.userauth_password(Some("demo"), Some("password"))?;
    let sftp = session.sftp()?;
    copy_folder(&sftp, Path::new("."), Path::new("data"), &mut map)?;
    anyhow::Ok(())
}

fn copy_folder(sftp: &Sftp, dirpath: &Path, prefix: &Path, map: &mut HashMap<PathBuf, u64>) -> anyhow::Result<()> {
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
        // Might make this a match statement to be cleaner
        if let libssh_rs::FileType::Directory = filetype {
            let dirname = f.name().ok_or(anyhow::anyhow!("Failed to get file name"))?;
            if dirname == "." || dirname == ".." {
                continue;
            }
            let prefixdir = &prefix.join(dirname);
            std::fs::create_dir_all(prefixdir)?;
            copy_folder(sftp, &dirpath.join(dirname), prefixdir, map)?;
        } else if let libssh_rs::FileType::Regular = filetype {
            let name = f.name().ok_or(anyhow::anyhow!("Failed to get file name"))?;
            println!("Name: {}", name);
            let rfilesize = f.len().ok_or(anyhow::anyhow!("Failed to get file size"))?;
            if let Some(fsize) = map.get(&prefix.join(name)) {
                if *fsize != rfilesize {
                    println!("Updated file");
                    let mut rfile = sftp.open(
                        &dirpath
                            .join(name)
                            .to_str()
                            .ok_or(anyhow::anyhow!("Failed to join remote file path"))?,
                        libssh_rs::OpenFlags::READ_ONLY,
                        0,
                    )?;
                    let mut file = std::fs::File::open(&prefix.join(name))?;
                    std::io::copy(&mut rfile, &mut file)?;
                }
            }
            else {
                println!("Copied file");
                let mut rfile = sftp.open(
                    &dirpath
                        .join(name)
                        .to_str()
                        .ok_or(anyhow::anyhow!("Failed to join remote file path"))?,
                    libssh_rs::OpenFlags::READ_ONLY,
                    0,
                )?;
                let fname = prefix.join(name);
                let mut file = std::fs::File::create(&fname)?;
                std::io::copy(&mut rfile, &mut file)?;
                map.insert(fname, rfilesize);
            }
        } else {
            return anyhow::Result::Err(anyhow::anyhow!("Invalid file type encountered"));
        }
    }
    anyhow::Ok(())
}

// Make it recursive
fn populate_map(path: &Path, map: &mut HashMap<PathBuf, u64>) -> std::io::Result<()> {
    let mut ret: std::io::Result<()> = Ok(());
    let dir = std::fs::read_dir(path)?.into_iter().collect::<std::io::Result<Box<[std::fs::DirEntry]>>>()?;
    for f in dir.into_iter() {
        let ftype = f.file_type()?;
        if ftype.is_dir() {
            populate_map(&path.join(f.file_name()), map)?;
        }
        else if ftype.is_file() {
            map.insert(path.join(f.file_name()), f.metadata()?.len());
        }
        else {
            ret = Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, ""));
        }
    }
    ret
}
