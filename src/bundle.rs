use anyhow::Result;
use rattler_package_streaming::seek::{stream_conda_content, stream_conda_info};
use serde::{Deserialize, Serialize};
use std::io::{Cursor, Read, Seek};

fn license_files_from_conda_package<R: Read + Seek>(mut reader: R) -> Result<Vec<LicenseFile>> {
    let mut license_files: Vec<LicenseFile> = Vec::new();

    let mut buffer = Vec::new();
    reader.read_to_end(&mut buffer)?;
    let reader = Cursor::new(buffer);

    let mut content_archive = stream_conda_content(reader.clone())?;
    let mut info_archive = stream_conda_info(reader)?;

    for entry in content_archive.entries()? {
        let mut file = entry?;
        let path = file.path()?.to_path_buf();
        if path.components().any(|c| c.as_os_str() == "licenses") {
            let mut content = String::new();
            file.read_to_string(&mut content)?;

            let license_file = LicenseFile {
                filename: path.file_name().unwrap().to_string_lossy().to_string(),
                license_text: content,
            };

            license_files.push(license_file);
        }
    }

    for entry in info_archive.entries()? {
        let mut file = entry?;
        let path = file.path()?.to_path_buf();
        if path.components().any(|c| c.as_os_str() == "licenses") {
            let mut content = String::new();
            file.read_to_string(&mut content)?;

            let license_file = LicenseFile {
                filename: path.file_name().unwrap().to_string_lossy().to_string(),
                license_text: content,
            };

            license_files.push(license_file);
        }
    }

    println!(
        "{:?}",
        license_files
            .iter()
            .map(|lf| lf.filename.as_str())
            .collect::<Vec<_>>()
    );

    Ok(license_files)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LicenseFile {
    filename: String,
    license_text: String,
}

#[cfg(test)]
mod tests {
    use std::{fs::File, path::Path};

    use super::*;

    #[test]
    fn test_streaming() {
        let path = Path::new("tests/pixi-0.44.0-h8176bc1_0.conda");
        license_files_from_conda_package(File::open(path).unwrap()).unwrap();
        // let content = stream_conda_info(File::open(path).unwrap()).unwrap();
        // let file = read_package_file::<LicenseFile>(path).unwrap();
        println!("This is the test");
    }
}
