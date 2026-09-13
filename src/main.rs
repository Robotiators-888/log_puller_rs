use libssh_rs::{Session, Sftp};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Mutex,
};

fn main() -> anyhow::Result<()> {
    let mut map: HashMap<PathBuf, u64> = HashMap::new();
    // Use notifications to deal with this later
    populate_map(Path::new("data"), &mut map);
    println!("{:?}", map);
    let map: dashmap::DashMap<PathBuf, u64> = map.into_iter().collect();
    let mut filesinfos: Vec<(PathBuf, PathBuf, u64)> = Vec::new();
    let session = Session::new()?;
    // session.set_option(libssh_rs::SshOption::Hostname(String::from("localhost")))?;
    session.set_option(libssh_rs::SshOption::Hostname(String::from(
        "test.rebex.net",
    )))?;
    session.connect()?;
    session.userauth_password(Some("demo"), Some("password"))?;
    let sftp = session.sftp()?;
    get_folder_info(&sftp, Path::new("."), Path::new("data"), &mut filesinfos)?;
    let msftp = Mutex::new(sftp);
    filesinfos
        .into_par_iter()
        .for_each(move |i| copy_file(&msftp, i, &map).unwrap());
    anyhow::Ok(())
}

// Arguments could be optimized
fn get_folder_info(
    sftp: &Sftp,
    dirpath: &Path,
    prefix: &Path,
    filesinfos: &mut Vec<(PathBuf, PathBuf, u64)>,
) -> anyhow::Result<()> {
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
        // If its a directory then call the function recursivley, copy if its a file, otherwise return an error
        // Might make this a match statement to be cleaner
        match f.file_type().ok_or(anyhow::anyhow!("Failed to get file type"))? {
            libssh_rs::FileType::Directory => {
                let dirname = f.name().ok_or(anyhow::anyhow!("Failed to get file name"))?;
                if dirname == "." || dirname == ".." {
                    continue;
                }
                let prefixdir = &prefix.join(dirname);
                std::fs::create_dir_all(prefixdir)?;
                get_folder_info(sftp, &dirpath.join(dirname), prefixdir, filesinfos)?;
            }
            libssh_rs::FileType::Regular => {
                let fname = f.name().ok_or(anyhow::anyhow!("Failed to get file name"))?;
                filesinfos.push((
                    prefix.join(fname),
                    dirpath.join(fname),
                    f.len().ok_or(anyhow::anyhow!("Failed to get file size"))?,
                ));
            }
            _ => return anyhow::Result::Err(anyhow::anyhow!("Invalid file type encountered"))
        }
    }
    anyhow::Ok(())
}

fn copy_file(
    msftp: &Mutex<Sftp>,
    filesinfo: (PathBuf, PathBuf, u64),
    map: &dashmap::DashMap<PathBuf, u64>,
) -> anyhow::Result<()> {
    let mut needs_copy = false;
    let mut new_file = false;
    if let Some(mut fsize) = map.get_mut(&filesinfo.0) {
        if *fsize != filesinfo.2 {
            *fsize = filesinfo.2;
            needs_copy = true;
        }
    } else {
        needs_copy = true;
        new_file = true;
    }
    if needs_copy { 
        if new_file {
            println!(
                "Copied file: {:?}, {:?}, {:?}",
                filesinfo.0, filesinfo.1, filesinfo.2
            );
            // Locking logic of map could probably still be more efficient
            let mut file = std::fs::File::create(&filesinfo.0)?; // Blocking io locking a mutex :(
            map.insert(filesinfo.0, filesinfo.2);
            let sftp = msftp.lock().expect("SFTP Mutex Poisoned");
            let mut rfile = sftp.open(
                &filesinfo
                    .1
                    .to_str()
                    .ok_or(anyhow::anyhow!("Failed to join remote file path"))?,
                libssh_rs::OpenFlags::READ_ONLY,
                0,
            )?;
            std::mem::drop(sftp);
            std::io::copy(&mut rfile, &mut file)?;
        }
        else {
            println!(
                "Updated file: {:?}, {:?}, {:?}",
                filesinfo.0, filesinfo.1, filesinfo.2
            );
            let mut file = std::fs::File::open(&filesinfo.0)?;
            let sftp = msftp.lock().expect("SFTP Mutex Poisoned");
            let mut rfile = sftp.open(
                &filesinfo
                    .1
                    .to_str()
                    .ok_or(anyhow::anyhow!("Failed to join remote file path"))?,
                libssh_rs::OpenFlags::READ_ONLY,
                0,
            )?;
            std::mem::drop(sftp);
            std::io::copy(&mut rfile, &mut file)?;
        }
    }
    anyhow::Ok(())
}

fn populate_map(path: &Path, map: &mut HashMap<PathBuf, u64>) -> std::io::Result<()> {
    let mut ret: std::io::Result<()> = Ok(());
    let dir = std::fs::read_dir(path)?
        .into_iter()
        .collect::<std::io::Result<Box<[std::fs::DirEntry]>>>()?;
    for f in dir.into_iter() {
        let ftype = f.file_type()?;
        if ftype.is_dir() {
            populate_map(&path.join(f.file_name()), map)?;
        } else if ftype.is_file() {
            map.insert(path.join(f.file_name()), f.metadata()?.len());
        } else {
            ret = Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid file type"));
        }
    }
    ret
}
