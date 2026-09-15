use libssh_rs::{Session, Sftp};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::{collections::HashMap, path::Path, sync::Mutex, time::Duration};

#[derive(Debug)]
enum AppError {
    Io(std::io::Error),
    Ssh(libssh_rs::Error),
    Custom(String),
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::Io(e) => write!(f, "IO Error: {}", e),
            AppError::Ssh(e) => write!(f, "SSH Error: {}", e),
            AppError::Custom(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for AppError {}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::Io(err)
    }
}

impl From<libssh_rs::Error> for AppError {
    fn from(err: libssh_rs::Error) -> Self {
        AppError::Ssh(err)
    }
}

impl From<&str> for AppError {
    fn from(err: &str) -> Self {
        AppError::Custom(err.to_string())
    }
}

fn main() -> Result<(), AppError> {
    let mut map: HashMap<Box<Path>, u64> = HashMap::new();

    if let Err(e) = populate_map(Path::new("data"), &mut map) {
        let _ = notify_rust::Notification::new()
            .summary("Error populating map")
            .body(&format!("This is completely okay (most of the time), this will always happen if the logs folder is empty\nError: {}", e))
            .show();
    }

    println!("Map: {:?}", map);
    let map: dashmap::DashMap<Box<Path>, u64> = map.into_iter().collect();

    let mut sleeptime: Duration;
    loop {
        if let Err(e) = pull_logs(&map) {
            eprintln!("{}", e);
            sleeptime = Duration::from_secs(3);
        } else {
            sleeptime = Duration::from_secs(60);
        }
        std::thread::sleep(sleeptime);
    }

    // Ok(())
}

fn pull_logs(map: &dashmap::DashMap<Box<Path>, u64>) -> Result<(), AppError> {
    let session = Session::new()?;
    session.set_option(libssh_rs::SshOption::Hostname(String::from(
        "test.rebex.net",
    )))?;
    session.set_option(libssh_rs::SshOption::User(Some(String::from("demo"))))?;
    session.connect()?;
    session.userauth_password(None, Some("password"))?;

    let sftp = session.sftp()?;
    let mut filesinfos: Vec<(Box<Path>, Box<Path>, u64)> = Vec::new();

    if let Err(e) = get_folder_info(&sftp, Path::new("."), Path::new("data"), &mut filesinfos) {
        notify_rust::Notification::new()
            .summary("Error getting folder info")
            .body(&format!("Error: {}", e))
            .show()
            .map_err(|n_err| AppError::Custom(n_err.to_string()))?;
    }

    let msftp = Mutex::new(sftp);
    let copy_result = filesinfos
        .into_par_iter()
        .map(move |i| copy_file(&msftp, i, &map))
        .collect::<Result<(), AppError>>();

    if let Err(e) = copy_result {
        notify_rust::Notification::new()
            .summary("Error copying files")
            .body(&format!("Error: {}", e))
            .show()
            .map_err(|n_err| AppError::Custom(n_err.to_string()))?;
    }

    Ok(())
}

fn get_folder_info(
    sftp: &Sftp,
    dirpath: &Path,
    prefix: &Path,
    filesinfos: &mut Vec<(Box<Path>, Box<Path>, u64)>,
) -> Result<(), AppError> {
    println!("Path: {}", dirpath.to_str().unwrap_or(""));
    println!("Prefix: {}", prefix.to_str().unwrap_or(""));

    let path_str = dirpath.to_str().ok_or("Couldn't convert path to str")?;

    for f in sftp.read_dir(path_str)?.into_iter() {
        let ftype = f.file_type().ok_or("Failed to get file type")?;
        match ftype {
            libssh_rs::FileType::Directory => {
                let dirname = f.name().ok_or("Failed to get file name")?;
                if dirname == "." || dirname == ".." {
                    continue;
                }
                let prefixdir = &prefix.join(dirname);
                std::fs::create_dir_all(prefixdir)?;
                get_folder_info(sftp, &dirpath.join(dirname), prefixdir, filesinfos)?;
            }
            libssh_rs::FileType::Regular => {
                let fname = f.name().ok_or("Failed to get file name")?;
                let f_len = f.len().ok_or("Failed to get file size")?;
                filesinfos.push((
                    prefix.join(fname).into_boxed_path(),
                    dirpath.join(fname).into_boxed_path(),
                    f_len,
                ));
            }
            _ => return Err("Invalid file type encountered".into()),
        }
    }
    Ok(())
}

fn copy_file(
    msftp: &Mutex<Sftp>,
    filesinfo: (Box<Path>, Box<Path>, u64),
    map: &dashmap::DashMap<Box<Path>, u64>,
) -> Result<(), AppError> {
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
        let remote_path_str = filesinfo
            .1
            .to_str()
            .ok_or("Failed to join remote file path")?;

        if new_file {
            println!(
                "Copied file: {:?}, {:?}, {:?}",
                filesinfo.0, filesinfo.1, filesinfo.2
            );
            let mut file = std::fs::File::create(&filesinfo.0)?;
            map.insert(filesinfo.0, filesinfo.2);

            let sftp = msftp.lock().map_err(|_| "SFTP Mutex Poisoned")?;
            let mut rfile = sftp.open(remote_path_str, libssh_rs::OpenFlags::READ_ONLY, 0)?;
            drop(sftp);
            std::io::copy(&mut rfile, &mut file)?;
        } else {
            println!(
                "Updated file: {:?}, {:?}, {:?}",
                filesinfo.0, filesinfo.1, filesinfo.2
            );
            let mut file = std::fs::File::create(&filesinfo.0)?;

            let sftp = msftp.lock().map_err(|_| "SFTP Mutex Poisoned")?;
            let mut rfile = sftp.open(remote_path_str, libssh_rs::OpenFlags::READ_ONLY, 0)?;
            drop(sftp);
            std::io::copy(&mut rfile, &mut file)?;
        }
    } else {
        println!(
            "Skipped file: {:?}, {:?}, {:?}",
            filesinfo.0, filesinfo.1, filesinfo.2
        );
    }
    Ok(())
}

fn populate_map(path: &Path, map: &mut HashMap<Box<Path>, u64>) -> std::io::Result<()> {
    for f in std::fs::read_dir(path)?.into_iter() {
        let f = f?;
        let ftype = f.file_type()?;
        if ftype.is_dir() {
            populate_map(&path.join(f.file_name()), map)?;
        } else if ftype.is_file() {
            map.insert(
                path.join(f.file_name()).into_boxed_path(),
                f.metadata()?.len(),
            );
        } else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Invalid file type",
            ));
        }
    }
    Ok(())
}
