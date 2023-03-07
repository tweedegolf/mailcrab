use std::{fs, path::Path};

fn main() {
    let path = "../frontend/dist";
    if !Path::new(&path).exists() {
        fs::create_dir_all(path).expect("Could not create a frontend/dist folder");
    }
}
