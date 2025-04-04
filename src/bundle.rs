use rattler_conda_types::package::AboutJson;
use rattler_conda_types::PackageRecord;
use rattler_package_streaming::seek::stream_conda_info;

fn bundle() {

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_streaming() {
        let path = Path::new("tests/polarify-0.2.0-pyhd8ed1ab_1.conda");
        let file = read_package_file(path).unwrap();
        println!("{:?", file);
    }
}