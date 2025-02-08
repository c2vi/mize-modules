// helper application for dev module... that is run by nix develop ... and then runns certain bash
// commands and tells the dev-module, when they are finished and the output

use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::thread;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use cmd_lib::run_cmd;
use mize::{mize_err, MizeResult};
use mize::Instance;
use std::fs::{self, File};
use mize::error::IntoMizeResult;
use mize::MizeError;
use std::fs::OpenOptions;

fn main() {
    let mut instance = Instance::new().expect("unable to create mize instance");

    if let Err(e) = main_with_error_handling(&mut instance) {
        instance.report_err(e);
    }

}

fn main_with_error_handling(instance: &mut Instance) -> MizeResult<()> {
    let stdin = io::stdin();

    let buildable_name = std::env::args().nth(1).expect("run without name of buildable as first arg");

    let pipe_path = PathBuf::from(instance.get("0/config/store_path")?.value_string()?).join("mize_dev_module").join("pipe");

    let pipe = OpenOptions::new().write(true).read(false).open(pipe_path.as_path())
        .mize_result_msg(format!("couldn't open dev module pipe at '{}'", pipe_path.display()));


    for line in stdin.lines() {
        if let Err(e) = line {
            instance.report_err(mize_err!("error reading a line from stdin: {}", e));
            continue;
        }

        let line = line.unwrap();


        match handle_line(instance, line, buildable_name.clone(), pipe_path.clone()) {
            Ok(true) => { break; },
            Ok(_) => {},
            Err(e) => {
                instance.report_err(e);
            }
        }
    }

    Ok(())
}

// returns wether the program should exit
fn handle_line(instance: &mut Instance, line: String, buildable_name: String, pipe_path: PathBuf) -> MizeResult<bool> {
    //let split_tmp = shell_words::split(&line)?;
    let split = line.split(" ");
    //let split = split_tmp.iter().map(|v|v.as_str());

    let mut pipe = OpenOptions::new().read(false).write(true).open(pipe_path.as_path())?;

    match split.clone().nth(0) {
        Some("Run") => {
            let encoded_string_to_run = split.clone().skip(1).collect::<Vec<&str>>().join(" ");
            let string_to_run = encoded_string_to_run.replace("\\n", "\n");

            pipe.write_all(format!("BuildOutput {} dev module: got Run: {}\n", buildable_name, encoded_string_to_run).as_bytes().as_ref())?;


            let mut child = Command::new("bash")
                .arg("-c")
                .arg(string_to_run)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?;


            // stdout thread
            let stdout = child.stdout.take().expect("child had no stdout");
            let pipe_path_clone = pipe_path.clone();
            let buildable_name_clone = buildable_name.clone();
            instance.spawn("dev_module_stdout", move || {
                let mut pipe = OpenOptions::new().read(false).write(true).open(pipe_path_clone)?;
                let mut stdout = BufReader::new(stdout);

                let mut buf = String::new();

                loop {
                    buf = "BuildOutput ".to_owned();
                    buf = buf + buildable_name_clone.as_str() + " ";

                    if stdout.read_line(&mut buf)? == 0 {
                        break;
                    };

                    println!("stdout got line: {}", buf);

                    pipe.write_all(&buf.as_bytes())?;
                }
                Ok(())
            });

            // stderr thread
            let stderr = child.stderr.take().expect("child had no stdout");
            let pipe_path_clone = pipe_path.clone();
            let buildable_name_clone = buildable_name.clone();
            instance.spawn("dev_module_stdout", move || {
                let mut pipe = OpenOptions::new().read(false).write(true).open(pipe_path_clone)?;
                let mut stderr = BufReader::new(stderr);

                let mut buf = String::new();

                loop {
                    buf = "BuildOutput ".to_owned();
                    buf = buf + buildable_name_clone.as_str() + " ";

                    if stderr.read_line(&mut buf)? == 0 {
                        break;
                    };

                    pipe.write_all(&buf.as_bytes())?;
                }
                Ok(())
            });

            child.wait()?;

            // send BuildFinished event
            let mut pipe = OpenOptions::new().read(false).write(true).open(pipe_path)?;
            pipe.write_all(format!("BuildFinished {}\n", buildable_name).as_bytes().as_ref())?;

        },

        Some("exit") => {
            println!("exiting...");
            return Ok(true);
        },

        Some(_) | None => {
            instance.report_err(mize_err!("got an invalid line on stdin. the line: '{}'", line));
        }
    }

    Ok(false)
}

