//! This crate contains an API for using the Zngur code generator inside build scripts. For more information
//! about the Zngur itself, see [the documentation](https://hkalbasi.github.io/zngur).

use std::{
    collections::HashMap,
    fs::{self, File, read_to_string},
    io::Write,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use zngur_generator::{LayoutPolicy, ParsedZngFile, ZngurGenerator};

#[must_use]
/// Builder for the Zngur generator.
///
/// Usage:
/// ```ignore
/// let crate_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
/// let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
/// Zngur::from_zng_file(crate_dir.join("main.zng"))
///     .with_cpp_file(out_dir.join("generated.cpp"))
///     .with_h_file(out_dir.join("generated.h"))
///     .with_rs_file(out_dir.join("generated.rs"))
///     .generate();
/// ```
pub struct Zngur {
    zng_file: PathBuf,
    h_file_path: Option<PathBuf>,
    cpp_file_path: Option<PathBuf>,
    rs_file_path: Option<PathBuf>,
    mangling_base: Option<String>,
    cpp_namespace: Option<String>,
}

impl Zngur {
    pub fn from_zng_file(zng_file_path: impl AsRef<Path>) -> Self {
        Zngur {
            zng_file: zng_file_path.as_ref().to_owned(),
            h_file_path: None,
            cpp_file_path: None,
            rs_file_path: None,
            mangling_base: None,
            cpp_namespace: None,
        }
    }

    pub fn with_h_file(mut self, path: impl AsRef<Path>) -> Self {
        self.h_file_path = Some(path.as_ref().to_owned());
        self
    }

    pub fn with_cpp_file(mut self, path: impl AsRef<Path>) -> Self {
        self.cpp_file_path = Some(path.as_ref().to_owned());
        self
    }

    pub fn with_rs_file(mut self, path: impl AsRef<Path>) -> Self {
        self.rs_file_path = Some(path.as_ref().to_owned());
        self
    }

    pub fn with_mangling_base(mut self, mangling_base: &str) -> Self {
        self.mangling_base = Some(mangling_base.to_owned());
        self
    }

    pub fn with_cpp_namespace(mut self, cpp_namespace: &str) -> Self {
        self.cpp_namespace = Some(cpp_namespace.to_owned());
        self
    }

    pub fn generate(self) {
        let mut parsed_no_layout = ParsedZngFile::parse(self.zng_file);
        let rust_src = parsed_no_layout.generate_use_stmt();
        println!("{rust_src}");
        let rs = self.rs_file_path.clone().unwrap();
        let parent = rs.parent().unwrap();
        dbg!(&parent);

        let pre_gen = &parent.join("pre_gen.rs");
        fs::write(pre_gen, rust_src).unwrap();
        let gp = parent.parent().unwrap().to_path_buf();
        dbg!(gp);
        let sizes = get_type_sizes(parent.parent().unwrap().to_path_buf());
        dbg!(sizes);
        let mut file = ZngurGenerator::build_from_zng(parsed_no_layout);

        let rs_file_path = self.rs_file_path.expect("No rs file path provided");
        let h_file_path = self.h_file_path.expect("No h file path provided");

        file.0.cpp_include_header_name = h_file_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();

        file.0.cpp_namespace = "rust".to_owned();

        if let Some(cpp_namespace) = self.cpp_namespace {
            file.0.mangling_base = cpp_namespace.clone();
            file.0.cpp_namespace = cpp_namespace;
        }

        if let Some(mangling_base) = self.mangling_base {
            file.0.mangling_base = mangling_base;
        }

        let cpp_namespace = file.0.cpp_namespace.clone();

        let (rust, mut h, mut cpp) = file.render();

        // TODO: Don't hard code namespace as "::rust" and remove this replace
        h = h
            .replace("rust::", &format!("{cpp_namespace}::"))
            .replace("namespace rust", &format!("namespace {cpp_namespace}"));
        cpp = cpp.map(|cpp| cpp.replace("rust::", &format!("{cpp_namespace}::")));

        File::create(rs_file_path)
            .unwrap()
            .write_all(rust.as_bytes())
            .unwrap();
        File::create(h_file_path)
            .unwrap()
            .write_all(h.as_bytes())
            .unwrap();
        if let Some(cpp) = cpp {
            let cpp_file_path = self.cpp_file_path.expect("No cpp file path provided");
            File::create(cpp_file_path)
                .unwrap()
                .write_all(cpp.as_bytes())
                .unwrap();
        }
    }
}

fn get_type_sizes(path: PathBuf) -> HashMap<String, LayoutPolicy> {
    let raw_output = std::process::Command::new("cargo")
        .current_dir(&path)
        .args(["+nightly", "rustc", "--", "-Z", "print-type-sizes"])
        .output()
        .expect("something");
    let output = str::from_utf8(&raw_output.stdout).unwrap();
    let maybe_new_sizes = str_to_typesizes(output);
    let size_info = match maybe_new_sizes.is_empty() {
        false => {
            let serialized = serde_json::to_string(&maybe_new_sizes).unwrap();
            let mem_layout = &path.join("mem_layout.json");
            fs::write(mem_layout, serialized).unwrap();
            maybe_new_sizes
        }
        true => {
            let data = read_to_string(&path.join("mem_layout.json")).unwrap();
            let deserialized = serde_json::from_str(&data).unwrap();
            deserialized
        }
    };
    size_info
}

fn str_to_typesizes(input: &str) -> HashMap<String, LayoutPolicy> {
    input
        .split_terminator("\n")
        .filter(|s| s.contains("type:"))
        .map(|s| {
            let mut parts = s.split_whitespace().skip(2).peekable();
            let mut name = parts.next().unwrap().to_string().clone();
            while let Some(p) = parts.peek()
                && p.parse::<u32>().is_err()
            {
                name.push(' ');
                name.push_str(parts.next().unwrap());
            }
            let size = parts.next().unwrap().parse::<usize>().unwrap();
            let align = parts.skip(2).next().unwrap().parse::<usize>().unwrap();
            (
                name[1..(name.len() - 2)].to_string(),
                LayoutPolicy::StackAllocated { size, align },
            )
        })
        .collect::<HashMap<_, _>>()
}

fn json_to_typesizes(path: &Path) -> HashMap<String, LayoutPolicy> {
    let data = read_to_string(path.join("mem_layout.json")).unwrap();
    serde_json::from_str(&data).unwrap()
}
