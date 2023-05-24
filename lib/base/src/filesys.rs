use arrayvec::ArrayString;
use directories::ProjectDirs;

const MAX_PATH_LEN: usize = 256;

pub enum FileSystemType {
    // read write has a higher priority, since it contains user modifications
    ReadWrite,
    // the read-only file system is the one shipped with the executables
    Read,
    // working directory
    Exec,
}

// either get the file system path by a specific named type
// or get the path of a specific index
// or allow any path
pub enum FileSystemPath {
    OfType(FileSystemType),
    Index(usize),
}

pub struct FileSystem {
    pub paths: Vec<String>,
    config_dir_index: usize,
    data_dir_index: usize,
    exec_dir_index: usize,
}

impl FileSystem {
    pub fn new() -> Self {
        let mut config_dir: String = String::new();
        if let Some(proj_dirs) = ProjectDirs::from("org", "", "DDNet") {
            config_dir = proj_dirs.config_dir().to_str().unwrap().to_string();
        }
        let mut paths: Vec<String> = Vec::new();
        paths.push(config_dir);
        let config_dir_index = paths.len() - 1;
        paths.push("data/".to_string());
        let data_dir_index = paths.len() - 1;
        if let Ok(exec_path) = std::env::current_dir() {
            paths.push(exec_path.to_str().unwrap().to_string());
        }
        // if worst case this is equal to the data dir
        let exec_dir_index = paths.len() - 1;
        Self {
            paths,
            config_dir_index,
            data_dir_index,
            exec_dir_index,
        }
    }

    pub fn get_path(&self, path: &str, fs_path: FileSystemPath) -> ArrayString<MAX_PATH_LEN> {
        let index: usize;
        match fs_path {
            FileSystemPath::OfType(of_type) => match of_type {
                FileSystemType::ReadWrite => index = self.config_dir_index,
                FileSystemType::Read => index = self.data_dir_index,
                FileSystemType::Exec => index = self.exec_dir_index,
            },
            FileSystemPath::Index(path_index) => index = path_index,
        }
        let mut res = ArrayString::<MAX_PATH_LEN>::from(&self.paths[index].as_str()).unwrap();
        res.push_str(path);
        res
    }

    pub async fn open_file(&self, file_path: &str) -> std::io::Result<Vec<u8>> {
        for (path_index, _) in self.paths.iter().enumerate() {
            let file = tokio::fs::read(
                self.get_path(file_path, FileSystemPath::Index(path_index))
                    .as_str(),
            )
            .await;
            if file.is_ok() {
                return file;
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file not found",
        ))
    }

    async fn files_of_dir_impl<'a, T>(
        &self,
        path: &str,
        file_read_cb: &'a mut T,
        fs_path: FileSystemPath,
        file_list: &mut Vec<String>,
    ) where
        T: FnMut(String, Vec<u8>) + Send + Sync,
    {
        let full_path = self.get_path(path, fs_path);
        let mut dir_read = tokio::fs::read_dir(full_path.as_str()).await;

        if let Ok(dir_reader) = &mut dir_read {
            while let Ok(Some(entry)) = dir_reader.next_entry().await {
                let file_type_res = entry.file_type().await;
                let file_name = entry.file_name().to_str().unwrap().to_string();
                if let Ok(file_type) = file_type_res {
                    if file_type.is_file() && !file_list.contains(&file_name) {
                        let file = tokio::fs::read(full_path.to_string() + &file_name).await;
                        if let Ok(f) = file {
                            file_list.push(file_name.clone());
                            file_read_cb(file_name, f);
                        }
                    }
                }
            }
        }
    }

    pub async fn files_of_dir<'a, T>(&self, path: &str, file_read_cb: &'a mut T)
    where
        T: FnMut(String, Vec<u8>) + Send + Sync,
    {
        let mut file_list = Vec::<String>::new();
        for (path_index, _) in self.paths.iter().enumerate() {
            self.files_of_dir_impl(
                path,
                file_read_cb,
                FileSystemPath::Index(path_index),
                &mut file_list,
            )
            .await;
        }
    }
}
