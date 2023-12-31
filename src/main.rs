use std::error::Error;
use std::fs::{create_dir, remove_dir_all, remove_file, File};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;
use std::{env, fs};

use toml::Value;

fn error_c(err: Option<Box<dyn Error>>) {
    if let Some(error) = err {
        eprintln!(
            "Error cannot find right configuration for this directory {}",
            error
        );
    } else {
        eprintln!("Error cannot find right configuration for this directory");
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if let Some(mode) = args.get(1) {
        let mut cargo_config_file: Option<Box<PathBuf>> = None;
        let mut cargo_crate_name: Option<String> = None;
        let mut test_args: Option<Vec<String>> = None;
        let mut run_args: Option<Vec<String>> = None;
        if let Ok(current_dir) = env::current_dir() {
            let config = current_dir.join(PathBuf::from(".cargo/config.toml"));
            let mut config_file = match File::open(config) {
                Ok(file) => file,
                Err(e) => {
                    error_c(Some(Box::from(e)));
                    return;
                }
            };

            let mut toml_string = String::new();
            if let Err(e) = config_file.read_to_string(&mut toml_string) {
                error_c(Some(Box::from(e)));
                return;
            }

            let toml_value: Value = match toml::de::from_str(&toml_string) {
                Ok(value) => value,
                Err(e) => {
                    error_c(Some(Box::from(e)));
                    return;
                }
            };

            if let Some(build_cfg) = toml_value.get("build") {
                if let Some(target) = build_cfg.get("target") {
                    if let Some(target_value) = target.as_str() {
                        cargo_config_file = Some(Box::new(
                            current_dir.join(PathBuf::from(target_value).as_path()),
                        ));
                    }
                }
            }
            let config = current_dir.join(PathBuf::from("Cargo.toml"));
            let mut config_file = match File::open(config) {
                Ok(file) => file,
                Err(e) => {
                    error_c(Some(Box::from(e)));
                    return;
                }
            };

            let mut toml_string = String::new();
            if let Err(e) = config_file.read_to_string(&mut toml_string) {
                error_c(Some(Box::from(e)));
                return;
            }

            let toml_value: Value = match toml::de::from_str(&toml_string) {
                Ok(value) => value,
                Err(e) => {
                    error_c(Some(Box::from(e)));
                    return;
                }
            };

            if let Some(package) = toml_value.get("package") {
                if let Some(name) = package.get("name") {
                    if let Some(name_value) = name.as_str() {
                        cargo_crate_name = Some(String::from(name_value));
                    }
                }
                if let Some(metadata) = package.get("metadata") {
                    if let Some(osc) = metadata.get("osc") {
                        if let Some(test_arg) = osc.get("test-args") {
                            if let Some(array) = test_arg.as_array() {
                                test_args = Some(
                                    array
                                        .iter()
                                        .map(|value| value.as_str().unwrap().to_string())
                                        .collect::<Vec<String>>(),
                                );
                            }
                        }
                        if let Some(run_arg) = osc.get("run-args") {
                            if let Some(array) = run_arg.as_array() {
                                run_args = Some(
                                    array
                                        .iter()
                                        .map(|value| value.as_str().unwrap().to_string())
                                        .collect::<Vec<String>>(),
                                );
                            }
                        }
                    }
                }
            }
        }
        if mode == "build" {
            if let Ok(current_dir) = env::current_dir() {
                let mut cargo = Command::new("cargo");
                cargo.arg("build").current_dir(current_dir.clone());
                for arg in args.iter().skip(2) {
                    cargo.arg(arg);
                }
                cargo.status().expect("Cargo build failed");
                if args
                    .iter()
                    .skip(2)
                    .find(|&value| value == "--release")
                    .is_some()
                {
                    if let Some(config_file) = cargo_config_file {
                        if let Some(config_file_name) = config_file.file_stem() {
                            if let Some(config_file_name_string) = config_file_name.to_str() {
                                if let Some(name) = cargo_crate_name {
                                    build_iso(
                                        &String::from(
                                            current_dir
                                                .join(PathBuf::from(format!(
                                                    "target/{}/release/{}",
                                                    config_file_name_string, name
                                                )))
                                                .to_str()
                                                .unwrap(),
                                        ),
                                        &name,
                                        &current_dir,
                                    );
                                }
                            }
                        }
                    }
                } else {
                    if let Some(config_file) = cargo_config_file {
                        if let Some(config_file_name) = config_file.file_stem() {
                            if let Some(config_file_name_string) = config_file_name.to_str() {
                                if let Some(name) = cargo_crate_name {
                                    build_iso(
                                        &String::from(
                                            current_dir
                                                .join(PathBuf::from(format!(
                                                    "target/{}/debug/{}",
                                                    config_file_name_string, name
                                                )))
                                                .to_str()
                                                .unwrap(),
                                        ),
                                        &name,
                                        &current_dir,
                                    );
                                }
                            }
                        }
                    }
                }
            }
        } else if mode == "runner" {
            if let Ok(current_dir) = env::current_dir() {
                if let Some(path) = args.get(2) {
                    if let Some(name) = cargo_crate_name {
                        let (iso, work_dir) = build_iso(path, &name, &current_dir);
                        if let Some(iso) = iso {
                            if let Some(work_dir) = work_dir {
                                let mut qemu = Command::new("qemu-system-x86_64");
                                qemu.arg("-cdrom")
                                    .arg(iso.to_str().unwrap())
                                    .current_dir(work_dir);
                                for arg in args.iter().skip(3) {
                                    qemu.arg(arg);
                                }
                                if path.starts_with(current_dir.to_str().unwrap()) {
                                    if let Some(testargs) = test_args {
                                        for arg in testargs.iter() {
                                            qemu.arg(arg);
                                        }
                                    }
                                } else {
                                    if let Some(runargs) = run_args {
                                        for arg in runargs.iter() {
                                            qemu.arg(arg);
                                        }
                                    }
                                }
                                qemu.status().expect("Qemu Failed");
                            }
                        }
                    }
                }
            }
        }
    }
}

fn build_iso(
    path: &String,
    cargo_crate_name: &String,
    current_dir_5555: &PathBuf,
) -> (Option<Box<Path>>, Option<Box<Path>>) {
    let mut path = path.clone();
    if !path.starts_with(current_dir_5555.to_str().unwrap()) {
        path.insert_str(
            0,
            format!("{}/", current_dir_5555.to_str().unwrap()).as_str(),
        );
    };
    let path = Path::new(&path);
    if path.parent().unwrap().file_name().unwrap() == "deps"
        && get_first_segment(path.file_name().unwrap().to_str().unwrap())
            == cargo_crate_name.as_str()
    {
        remove_file(path).unwrap();
        remove_file(path.to_str().unwrap().to_owned() + ".d").unwrap();
        return (None, None);
    }

    let mut prefix = path.file_name().unwrap().to_str().unwrap();
    if path.parent().unwrap().file_name().unwrap() != "deps" {
        prefix = get_first_segment(path.file_name().unwrap().to_str().unwrap());
    }
    if let Some(mut directory) = path.parent() {
        if path.parent().unwrap().file_name().unwrap() == "deps" {
            directory = directory.parent().unwrap();
        }
        let deps_dir = directory.join(PathBuf::from("deps"));
        let work_dir = directory
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        let mut index = 0;
        while {
            if directory.join(PathBuf::from("build-temp")).exists() {
                remove_dir_all(directory.join(PathBuf::from("build-temp"))).unwrap();
            }
            create_dir(directory.join(PathBuf::from("build-temp"))).unwrap();

            get_object(
                index,
                directory,
                path,
                deps_dir.clone(),
                prefix,
                work_dir,
                cargo_crate_name,
            );
            let object_files: Vec<String> =
                fs::read_dir(directory.join(PathBuf::from("build-temp")))
                    .expect("Failed to read object directory")
                    .filter_map(|entry| {
                        if let Ok(entry) = entry {
                            if let Some(extension) = entry.path().extension() {
                                if extension == "o" {
                                    Some(entry.path().to_string_lossy().into_owned())
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    })
                    .collect();
            let mut command = std::process::Command::new("ld");
            command
                .arg("-n")
                .arg("--gc-sections")
                .arg("-o")
                .arg(
                    work_dir.join(
                        PathBuf::from("iso")
                            .join(PathBuf::from("boot").join(PathBuf::from("kernel.bin"))),
                    ),
                )
                .arg("-T")
                .arg(
                    current_dir_5555
                        .join(PathBuf::from("linker.ld"))
                        .to_str()
                        .unwrap(),
                )
                .current_dir(directory.join(PathBuf::from("build-temp")))
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            for object_file in &object_files {
                command.arg(object_file);
            }
            let status = command.status().expect("Failed to run linker");
            !status.success()
        } {
            std::thread::sleep(Duration::from_secs(1));
            index += 1;
        }
        let mut iso_grub = Command::new("grub-mkrescue");
        iso_grub
            .arg("-o")
            .arg("os.iso")
            .arg("iso")
            .current_dir(work_dir)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        iso_grub.status().expect("Failed to build iso");
        return (
            Some(Box::from(work_dir.join(PathBuf::from("os.iso").as_path()))),
            Some(Box::from(work_dir)),
        );
    }
    return (None, None);
}

fn get_files_with_extension_and_prefix(
    directory_path: &Path,
    extension: &str,
    prefix: &str,
) -> Vec<std::path::PathBuf> {
    let mut result = Vec::new();

    if let Ok(entries) = fs::read_dir(directory_path) {
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_file() {
                    if let Some(file_name) = entry.file_name().to_str() {
                        if file_name.starts_with(prefix) && file_name.ends_with(extension) {
                            result.push(entry.path());
                        }
                    }
                }
            }
        }
    }

    result
}

fn get_object(
    index: usize,
    directory: &Path,
    path: &Path,
    deps_dir: PathBuf,
    prefix: &str,
    work_dir: &Path,
    cargo_crate_name: &String,
) {
    get_lib(directory, path, deps_dir.clone(), cargo_crate_name);
    get_asm(work_dir, directory);
    get_bin(index, deps_dir, prefix, directory);
}

fn get_asm(work_dir: &Path, directory: &Path) {
    let asm_files: Vec<String> =
        fs::read_dir(work_dir.join(PathBuf::from("src").join(PathBuf::from("boot"))))
            .expect("Failed to read object directory")
            .filter_map(|entry| {
                if let Ok(entry) = entry {
                    if let Some(extension) = entry.path().extension() {
                        if extension == "asm" {
                            Some(entry.path().to_string_lossy().into_owned())
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();
    for asm_file in &asm_files {
        let mut command = std::process::Command::new("nasm");
        command.arg("-felf64").arg(asm_file).arg("-o").arg(
            directory.join(
                PathBuf::from("build-temp").join(PathBuf::from(
                    String::from(
                        PathBuf::from(asm_file)
                            .file_name()
                            .unwrap()
                            .to_str()
                            .unwrap(),
                    )
                    .replace("asm", "o"),
                )),
            ),
        );
        command.status().expect("Failed to run nasm");
    }
}

fn get_lib(directory: &Path, path: &Path, deps_dir: PathBuf, cargo_crate_name: &String) {
    let d_files = get_files_with_extension_and_prefix(&deps_dir, ".d", cargo_crate_name);

    if path.parent().unwrap().file_name().unwrap() != "deps" {
        for file in d_files {
            let file = File::open(file).unwrap();
            let reader = BufReader::new(file);

            let lines: Vec<String> = reader.lines().map(|line| line.unwrap()).collect();
            let mut files_i_d = Vec::new();
            for line in lines.iter().rev() {
                if line.is_empty() {
                    break;
                }
                files_i_d.push(line);
            }
            if files_i_d.iter().rev().collect::<Vec<_>>()[0].as_str() == "src/lib.rs:" {
                let mut is_n = true;
                for line in lines.iter() {
                    if line.starts_with("src/serial.rs:") {
                        is_n = false;
                    }
                }
                if is_n {
                    for line in lines.iter() {
                        if line.starts_with("src") {
                            break;
                        }

                        let file = Path::new(seperate_path_makefile(line));
                        if seperate_path_makefile(line).ends_with(".a") {
                            fs::copy(
                                file,
                                Path::new(&directory.join(
                                    PathBuf::from("build-temp").join(file.file_name().unwrap()),
                                )),
                            )
                            .unwrap();
                            extract_static_library(
                                Path::new(&directory.join(
                                    PathBuf::from("build-temp").join(file.file_name().unwrap()),
                                ))
                                .to_str()
                                .unwrap(),
                                directory
                                    .join(PathBuf::from("build-temp"))
                                    .to_str()
                                    .unwrap(),
                            );
                        }
                    }
                }
            }
        }
    } else {
        for file in d_files {
            let file = File::open(file).unwrap();
            let reader = BufReader::new(file);

            let lines: Vec<String> = reader.lines().map(|line| line.unwrap()).collect();
            let mut files_i_d = Vec::new();
            for line in lines.iter().rev() {
                if line.is_empty() {
                    break;
                }
                files_i_d.push(line);
            }
            if files_i_d.iter().rev().collect::<Vec<_>>()[0].as_str() == "src/lib.rs:" {
                for line in lines.iter() {
                    if line.starts_with("src/serial.rs:") {
                        for line in lines.iter() {
                            if line.starts_with("src") {
                                break;
                            }

                            let file = Path::new(seperate_path_makefile(line));
                            if seperate_path_makefile(line).ends_with(".a") {
                                fs::copy(
                                    file,
                                    Path::new(&directory.join(
                                        PathBuf::from("build-temp").join(file.file_name().unwrap()),
                                    )),
                                )
                                .unwrap();
                                extract_static_library(
                                    Path::new(&directory.join(
                                        PathBuf::from("build-temp").join(file.file_name().unwrap()),
                                    ))
                                    .to_str()
                                    .unwrap(),
                                    directory
                                        .join(PathBuf::from("build-temp"))
                                        .to_str()
                                        .unwrap(),
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
fn get_bin(index: usize, deps_dir: PathBuf, prefix: &str, directory: &Path) {
    let d_files = get_files_with_extension_and_prefix(&deps_dir, ".d", prefix);
    let mut files: Vec<Box<Path>> = Vec::new();
    for file in d_files {
        let file = File::open(file).unwrap();
        let reader = BufReader::new(file);

        let lines: Vec<String> = reader.lines().map(|line| line.unwrap()).collect();
        let mut files_i_d = Vec::new();
        for line in lines.iter().rev() {
            if line.is_empty() {
                break;
            }
            files_i_d.push(line);
        }
        if files_i_d.iter().rev().collect::<Vec<_>>()[0].as_str() != "src/lib.rs:" {
            for line in lines.iter() {
                if line.starts_with("src") {
                    break;
                }
                let file = Path::new(seperate_path_makefile(line));
                if seperate_path_makefile(line).ends_with(".o") {
                    files.push(Box::from(file));
                }
            }
        }
    }
    if let Some(file) = files.get(index) {
        fs::copy(
            file,
            Path::new(&directory.join(PathBuf::from("build-temp").join(file.file_name().unwrap()))),
        )
        .unwrap();
    }
}

fn extract_static_library(library_path: &str, output_directory: &str) {
    // Create the output directory if it doesn't exist
    std::fs::create_dir_all(output_directory).expect("Failed to create output directory");

    // Use the ar command to extract the .a file into the output directory
    Command::new("ar")
        .args(&["x", library_path])
        .current_dir(output_directory)
        .status()
        .expect("Failed to execute ar command");
}

fn get_first_segment(input: &str) -> &str {
    if let Some(index) = input.rfind('-') {
        &input[0..index]
    } else {
        input
    }
}

fn seperate_path_makefile(input: &str) -> &str {
    if let Some(index) = input.rfind(':') {
        &input[0..index]
    } else {
        input
    }
}
