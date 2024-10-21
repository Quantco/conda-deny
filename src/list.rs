use crate::license_info::LicenseInfos;

pub fn list_license_infos(license_infos: &LicenseInfos, colored: bool) -> String {
    let mut output = String::new();

    for license_info in &license_infos.license_infos {
        output.push_str(&license_info.pretty_print(colored));
    }
    output
}
