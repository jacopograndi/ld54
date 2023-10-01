#!/usr/bin/env -S cargo +nightly -Zscript

use std::{
    env, io,
    process::{Command, Stdio},
};

const ITCH_CHANNEL: &str = ":wasm";

fn get_crate_name() -> String {
    env::current_dir()
        .expect("no current dir")
        .file_name()
        .expect("no file name")
        .to_str()
        .expect("failed to turn dir name to string")
        .to_string()
}

enum Param {
    Optimize,
    PushToItch,
}
impl Param {
    fn is_set(&self) -> bool {
        let args: Vec<String> = env::args().collect();
        let (short, long) = self.to_short_long_string();
        return args.contains(&short) || args.contains(&long);
    }
    fn to_short_long_string(&self) -> (String, String) {
        let (short, long) = match &self {
            Self::Optimize => ("-o", "--optimize"),
            Self::PushToItch => ("-p", "--push-to-itch"),
        };
        (short.to_string(), long.to_string())
    }
}

fn run(name: &str, args: &[&str]) -> Result<(), io::Error> {
    let child = Command::new(name)
        .args(args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;
    child.wait_with_output()?;
    Ok(())
}

/// make a wasm build and push it to itch.io
fn main() -> Result<(), io::Error> {
    run(
        "cargo",
        &["build", "--target", "wasm32-unknown-unknown", "--release"],
    )?;
    run(
        "wasm-bindgen",
        &[
            "--out-dir",
            "./out/",
            "--target",
            "web",
            &("./target/wasm32-unknown-unknown/release/".to_string() + &get_crate_name() + ".wasm"),
        ],
    )?;
    if Param::Optimize.is_set() {
        run(
            "wasm-opt",
            &[
                "-Os",
                "out/rstack_bg.wasm",
                "-o",
                &("out/".to_string() + &get_crate_name() + ".wasm"),
            ],
        )?;
    }
    run("cp", &["-r", "assets/", "out/"])?;
    run("zip", &["-r", "out.zip", "out"])?;
    if Param::PushToItch.is_set() {
        run(
            "butler",
            &[
                "push",
                "out.zip",
                &("zjikra/zero-percent".to_string() + ITCH_CHANNEL),
            ],
        )?;
    }
    Ok(())
}
