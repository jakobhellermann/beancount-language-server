use std::path::{Path, PathBuf};
use std::str::FromStr;

pub trait ToFilePath {
    fn to_file_path(&self) -> Result<PathBuf, ()>;
}

impl ToFilePath for lsp_types::Uri {
    fn to_file_path(&self) -> Result<PathBuf, ()> {
        let url = url::Url::from_str(self.as_str()).map_err(|_| ())?;
        url.to_file_path()
    }
}

pub fn path_to_uri(file: &Path) -> lsp_types::Uri {
    let url = url::Url::from_file_path(file).expect("Failed to convert file path to URI");
    lsp_types::Uri::from_str(url.as_str()).expect("Failed to parse URL as LSP URI")
}
