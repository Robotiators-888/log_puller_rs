use libssh_rs::{Session, Sftp};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::{collections::HashMap, path::Path, time::Duration};

static SHOULD_EXIT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[derive(Debug)]
enum AppError {
    Io(std::io::Error),
    Ssh(libssh_rs::Error),
    Custom(String),
    Exit
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::Io(e) => write!(f, "IO Error: {}", e),
            AppError::Ssh(e) => write!(f, "SSH Error: {}", e),
            AppError::Custom(msg) => write!(f, "{}", msg),
            AppError::Exit => write!(f, "Exit"),
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
    ctrlc::set_handler(move || {
        SHOULD_EXIT.store(true, std::sync::atomic::Ordering::SeqCst);
    }).map_err(|e| AppError::Custom(e.to_string()))?;

    let mut map: HashMap<Box<Path>, u64> = HashMap::new();

    if let Err(e) = populate_map(Path::new("data"), &mut map) {
        let err = notify_rust::Notification::new()
            .summary("Error populating map")
            .body(&format!("This is completely okay (most of the time), this will always happen if the logs folder is empty\nError: {}", e))
            .show();
        if let Err(n) = err {
            eprintln!("{}", n);
        }
    }

    println!("Map: {:?}", map);
    let map: dashmap::DashMap<Box<Path>, u64> = map.into_iter().collect();

    let mut sleeptime: Duration;
    loop {
        if let Err(e) = pull_logs(&map) {
            if let AppError::Exit = e {
                let _ = notify_rust::Notification::new()
                    .summary("Exiting cleanly")
                    .show()
                    .map_err(|n_err| { eprintln!("{}", n_err.to_string()); AppError::Exit });
                println!("Exiting cleanly");
                break;
            }
            eprintln!("Failed to pull logs");
            eprintln!("{}", e);
            sleeptime = Duration::from_secs(3);
        } else {
            println!("Successfully pulled logs");
            sleeptime = Duration::from_secs(60);
        }
        std::thread::sleep(sleeptime);
    }

    Ok(())
}

fn create_sftp_session() -> Result<Sftp, AppError> {
    let session = Session::new()?;
    session.set_option(libssh_rs::SshOption::Hostname(String::from("10.8.88.2")))?;
    session.set_option(libssh_rs::SshOption::User(Some(String::from("admin"))))?;
    session.connect()?;
    session.userauth_password(None, Some(""))?;

    let sftp = session.sftp()?;
    Ok(sftp)
}

fn pull_logs(map: &dashmap::DashMap<Box<Path>, u64>) -> Result<(), AppError> {
    if SHOULD_EXIT.load(std::sync::atomic::Ordering::SeqCst) {
        return Err(AppError::Exit);
    }
    let sftp = create_sftp_session()?;
    let mut filesinfos: Vec<(Box<Path>, Box<Path>, u64)> = Vec::with_capacity(map.len());

    std::fs::create_dir_all("data")?;
    if let Err(e) = get_folder_info(
        &sftp,
        Path::new("/media/sda1/logs"),
        Path::new("data"),
        &mut filesinfos,
    ) {
        notify_rust::Notification::new()
            .summary("Error getting folder info")
            .body(&format!("Error: {}", e))
            .show()
            .map_err(|n_err| AppError::Custom(n_err.to_string()))?;
    }
    drop(sftp);

    let copy_result = filesinfos
        .into_par_iter()
        .map(move |i| copy_file(i, &map))
        .collect::<Result<(), AppError>>();

    if let Err(e) = copy_result {
        if let AppError::Exit = e {
            return Err(e);
        }
        else {
            notify_rust::Notification::new()
                .summary("Error copying files")
                .body(&format!("Error: {}", e))
                .show()
                .map_err(|n_err| AppError::Custom(n_err.to_string()))?;
        }
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

    let path_str = dirpath
        .to_str()
        .ok_or("Couldn't convert path to str")?
        .replace("\\", "/");

    for f in sftp.read_dir(&path_str)?.into_iter() {
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
    filesinfo: (Box<Path>, Box<Path>, u64),
    map: &dashmap::DashMap<Box<Path>, u64>,
) -> Result<(), AppError> {
    if SHOULD_EXIT.load(std::sync::atomic::Ordering::SeqCst) {
        return Err(AppError::Exit);
    }
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
        let sftp = create_sftp_session()?;
        let remote_path_str = filesinfo
            .1
            .to_str()
            .ok_or("Failed to join remote file path")?
            .replace("\\", "/");

        if new_file {
            println!(
                "Copied file: {:?}, {:?}, {:?}",
                filesinfo.0, filesinfo.1, filesinfo.2
            );
            let mut file = std::fs::File::create(&filesinfo.0)?;
            map.insert(filesinfo.0, filesinfo.2);

            let mut rfile = sftp.open(&remote_path_str, libssh_rs::OpenFlags::READ_ONLY, 0)?;
            std::io::copy(&mut rfile, &mut file)?;
        } else {
            println!(
                "Updated file: {:?}, {:?}, {:?}",
                filesinfo.0, filesinfo.1, filesinfo.2
            );
            let mut file = std::fs::File::create(&filesinfo.0)?;

            let mut rfile = sftp.open(&remote_path_str, libssh_rs::OpenFlags::READ_ONLY, 0)?;
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
