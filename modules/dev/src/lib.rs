
// to check if a src changed: find . -not \( -path ./target -prune \) -newermt "2025-01-08 17:26:10"
#![ allow( warnings ) ]

use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::fs::File;
use std::io::stdout;
use std::io::Read;
use std::io::Stdin;
use std::io::Write;
use std::path::PathBuf;
use std::process::Child;
use std::process::ChildStderr;
use std::process::ChildStdin;
use std::process::ChildStdout;
use std::process::Command;
use std::io::{ BufRead, BufReader };
use std::sync::atomic::AtomicBool;
use std::thread;
use clap::Command as ClapCommand;
use cmd_lib::run_cmd;
use crossterm::event;
use flume::Sender;
use libc::dev_t;
use mize::error::IntoMizeResult;
use mize::item::data_from_string;
use tui::TuiState;
use std::process::Stdio;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;
use colored::Colorize;
use mize::item::ItemData;
use mize::instance;
use mize::mize_err;
use mize::MizeResult;
use mize::MizeError;
use mize::Instance;
use mize::Module;
use cmd_lib::run_fun;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use ciborium::Value as CborValue;
use clap::ArgMatches;
use clap::Arg;
use crate::tui::Tui;


pub mod tui;

#[derive(Clone)]
pub struct DevModuleMutexed {
    inner: Arc<Mutex<DevModule>>,
}

struct MyChild {
    child: Child,
    stdin: Arc<Mutex<ChildStdin>>,
    stdout: Arc<Mutex<BufReader<ChildStdout>>>,
    stderr: Arc<Mutex<BufReader<ChildStderr>>>,
}

pub struct DevModule {
    dev_shells: HashMap<String, MyChild>,
    outputs: HashMap<String, Vec<String>>,
    state: HashMap<String, String>,
    data: DevModuleData,
    instance: Instance,
    tui_state: Option<TuiState>,
    event_rx: flume::Receiver<DevModuleEvent>,
    event_tx: flume::Sender<DevModuleEvent>,
}

pub enum DevModuleEvent {
    Term(crossterm::event::Event),
    BuildFinished(String),
    BuildOutput((String, String))
}

#[derive(Serialize, Deserialize)]
pub struct DevModuleData {
    last_build: u32,
    mize_flake: String,
    buildables: Vec<Buildable>,
    config: ItemData,
}

#[derive(Serialize, Deserialize, Default)]
pub struct Buildable {
    name: String,
    active: bool,
    config: ItemData,
    src_path: PathBuf,
    command: String,
}

#[no_mangle]
extern "C" fn get_mize_module_dev(empty_module: &mut Box<dyn Module + Send + Sync>, mize: Instance) -> () {
    let module = DevModule::new(mize);
    let new_box: Box<dyn Module + Send + Sync> = Box::new( DevModuleMutexed { inner: Arc::new(Mutex::new(module)) } );

    *empty_module = new_box
}

impl mize::Module for DevModuleMutexed {
    fn init(&mut self, instance: &Instance) -> MizeResult<()> {

        Ok(())
    }

    fn exit(&mut self, instance: &Instance) -> MizeResult<()> {

        Ok(())
    }

    fn clone_module(&self) -> Box<dyn Module + Send + Sync> {
        Box::new(self.clone())
    }

    fn run_cli(&mut self, instance: &Instance, cmd_line: Vec<OsString>) -> Option<MizeResult<()>> {

        let mut inner = match self.inner.lock() {
            Ok(val) => val,
            Err(err) => {
                return Some(Err(err.into()));
            }
        };

        return Some(inner.run_cli(cmd_line));
    }

}

impl DevModule {
    pub fn new(instance: Instance) -> DevModule {
        let (tx, rx) = flume::unbounded();
        DevModule {
            data: DevModuleData::default(),
            dev_shells: HashMap::new(),
            outputs: HashMap::new(),
            state: HashMap::new(),
            instance,
            tui_state: None,
            event_rx: rx,
            event_tx: tx,
        }
    }

    pub fn run_cli(&mut self, cmd_line: Vec<OsString>) -> MizeResult<()> {
        let cli = ClapCommand::new("dev")
            .about("The MiZe dev module")
            .subcommand(
                ClapCommand::new("add")
                .aliases(["a"])
                .about("add a module")
                .arg(Arg::new("name")
                    .help("the name of the module to build")
                )
                .arg(Arg::new("src-path")
                    .help("the source path or the module")
                )
                .arg(Arg::new("config")
                    .help("the modName or build-config of the module")
                    //.required(true)
                )
                .arg(Arg::new("system")
                    .long("system")
                    .short('s')
                    .help("the modName or build-config of the module")
                )
            )
            .subcommand(
                ClapCommand::new("remove")
                .aliases(["rm", "r"])
                .about("remove a module")
                .arg(Arg::new("name")
                    .help("the name of the module to build")
                )
            )
            .subcommand(
                ClapCommand::new("list")
                .aliases(["ls", "l"])
                .about("list all modules/buildables in current dev env")
            )
            .subcommand(
                ClapCommand::new("shell")
                .aliases(["s"])
                .about("run a shell instead of the tui")
            )
            .subcommand(
                ClapCommand::new("print")
                .aliases(["p"])
                .about("print info about a buildable")
                .arg(Arg::new("name")
                    .help("the name of the module to print info about")
                )
            )
        ;
        let cmd: Vec<&str> = cmd_line.iter().map(|s|s.to_str().expect("")).collect();
        println!("cmdline: {:?}", cmd);
        let matches = cli.get_matches_from(cmd_line);

        match matches.subcommand() {
            Some(("add", sub_matches)) => self.cmd_add_buildable(sub_matches)?,
            Some(("remove", sub_matches)) => self.cmd_remove_buildable(sub_matches)?,
            Some(("list", sub_matches)) => self.cmd_list_buildable(sub_matches)?,

            Some(("shell", sub_matches)) => self.run_shell()?,
            Some(("print", sub_matches)) => self.cmd_print_buildable(sub_matches)?,
            Some((cmd, _)) => { return Err(mize_err!("unknown subcommand '{}'", cmd));},
            None => self.run_tui()?,
        };

        Ok(())
    }


    fn tui_state(&mut self) -> &mut TuiState {

        Tui::init_state(self);

        return self.tui_state.as_mut().unwrap();
    }


    fn run_shell(&mut self) -> MizeResult<()> {

        ///////////////// spawn the shell to run commands from
        let shell = std::env::var("SHELL")?;
        // get PS1, may only work for bash
        let old_ps1 = run_fun!(bash --login  -c "echo $$PS1")?;
        //std::env::set_var("PS1", format!("{} {old_ps1} ", "(mize dev)".bright_green() ));

        let dev_shell_proc = Command::new(shell)
            .arg("-c")
            .arg(format!(r#"bash --init-file <(echo 'source $HOME/.bashrc; export PS1="{} {old_ps1} "')"#, "(mize dev)".bright_green())) // wow that's a pfusch
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .output().expect("failed to spawn sub shell");

        Ok(())
    }



    fn run_tui(&mut self) -> MizeResult<()> {

        self.load_data()?;

        self.start_pipe_listening_thread()?;

        self.start_dev_shells()?;

        let mut tui = tui::Tui::new(self)?;

        tui.run()?;
        
        self.store_data()?;

        Ok(())
    }

    fn start_pipe_listening_thread(&mut self) -> MizeResult<()> {

        let pipe_path = PathBuf::from(self.instance.get("0/config/store_path")?.value_string()?).join("mize_dev_module").join("pipe");
        if !pipe_path.exists() {
            run_cmd!("mkfifo" "$pipe_path")?;
        }

        let pipe = BufReader::new(File::open(pipe_path)?);
        let mut instance_clone = self.instance.clone();
        let event_tx_clone = self.event_tx.clone();
        self.instance.spawn("dev_module_pipe_listener", move || {
            for line in pipe.lines() {
                match line {
                    Ok(line) => {
                        if let Err(e) = handle_line(event_tx_clone.clone(), &mut instance_clone, line) {
                            instance_clone.report_err(e);
                        }
                    },
                    Err(e) => {
                        instance_clone.report_err(<std::io::Error as Into<MizeError>>::into(e).msg("couldn't read line from dev module pipe"));
                    }
                }

            }
            Ok(())
        });

        Ok(())
    }


    fn start_dev_shells(&mut self) -> MizeResult<()> {


        let host_system = run_fun!(nix eval --impure --expr "builtins.currentSystem")?;

        ///////////////// spawn dev shell for all Buildables
        for buildable in &self.data.buildables {
            let target_system = buildable.config.get_path("system")
                .map_err(|e| e.msg("The config of the Buildable does not have a system attribute"))?
                .value_string()?;

            let mod_name = buildable.config.get_path("modName")
                .map_err(|e| e.msg("The config of the Buildable does not have a modName attribute"))?
                .value_string()?;

            println!("self.data.mize_flake: {}", self.data.mize_flake);

            //let dev_shell_path = format!("{}#packages.{}.mizeFor.{}.modules.{}.devShell", data.mize_flake, host_system, target_system, mod_name);
            let dev_shell_path = format!("{}#packages.{}.mizeFor.{}.modules.{}.devShell", self.data.mize_flake, host_system, target_system, mod_name);
            println!("dev_shell_path: {}", dev_shell_path);

            let mut args = vec!["develop".to_owned(), "--impure".to_owned(), dev_shell_path];

            // add --override-input arg, when we have a config/override_nixpkgs set
            if let Ok(nixpkgs_path) = self.data.config.get_path("override_nixpkgs") {
                let arg = format!("--override-input nixpkgs {}", nixpkgs_path.value_string()?);
                args.push(arg);
            }

            // 
            args.push("-c".to_owned());
            args.push("bash".to_owned());
            args.push("-i".to_owned());

            let mut child = Command::new("nix")
                .args(args)
                .env("MIZE_MODULE_NO_REPO", "1")
                .env("MIZE_MODULE_NO_EXTERNALS", "1")
                .env("MIZE_MODULE_PATH", &buildable.src_path)
                .stdin(Stdio::piped())
                //.stdout(Stdio::piped())
                //.stderr(Stdio::piped())
                .spawn()?
            ;

            let my_child = MyChild {
                stdin: Arc::new(Mutex::new(child.stdin.take().unwrap())),
                stdout: Arc::new(Mutex::new(BufReader::new(child.stdout.take().unwrap()))),
                stderr: Arc::new(Mutex::new(BufReader::new(child.stderr.take().unwrap()))),
                child,
            };

            self.dev_shells.insert(buildable.name.clone(), my_child);
        }

        Ok(())
    }

    pub fn load_data(&mut self) -> MizeResult<()> {
        let data_path = PathBuf::from(self.instance.get("0/config/store_path")?.value_string()?).join("mize_dev_module").join("data.json");
        println!("data_path: {}", data_path.display());

        if !data_path.exists() {
            fs::create_dir_all(data_path.as_path().parent().ok_or(mize_err!(""))?)?;
            let string = serde_json::to_string(&DevModuleData::default())?;
            fs::write(&data_path, string);
        };
        self.data = serde_json::from_reader(File::open(&data_path)?)
            .map_err(|e| mize_err!("Failed to read mize dev module's data from mize_store_dir/mize_dev_module/data.json: {e}"))?;

        Ok(())

    }


    pub fn store_data(&self) -> MizeResult<()> {
        let data_path = PathBuf::from(self.instance.get("0/config/store_path")?.value_string()?).join("mize_dev_module").join("data.json");

        serde_json::to_writer_pretty(File::create(&data_path)?, &self.data)
            .mize_result_msg("failed to encode DevModuleData into json with serde")?;

        Ok(())
    }


    pub fn cmd_print_buildable(&mut self, sub_matches: &ArgMatches) -> MizeResult<()> {
        self.load_data()?;

        let name = sub_matches.get_one::<String>("name")
            .ok_or(mize_err!("no name argument"))?;

        let buildable = self.data.buildables.iter().find(|b| &b.name == name)
            .ok_or(mize_err!("no buildable with name '{}' found in current dev environment", name))?;

        println!("name: {}", name);

        println!("active: {}", buildable.active);

        println!("src_path: {}", buildable.src_path.display());

        println!("\nconfig: \n{}\n", buildable.config);

        println!("command: \n{}", buildable.command);


        Ok(())
    }


    pub fn cmd_list_buildable(&mut self, sub_matches: &ArgMatches) -> MizeResult<()> {
        self.load_data()?;

        println!("Buildables:");
        for buildable in &self.data.buildables {
            println!("{} \t\t active:{} \t\t at:{}", buildable.name, buildable.active, buildable.src_path.display())
        }

        Ok(())
    }


    pub fn cmd_add_buildable(&mut self, sub_matches: &ArgMatches) -> MizeResult<()> {
        self.load_data()?;

        let name = sub_matches.get_one::<String>("name")
            .ok_or(mize_err!("no name argument"))?;

        let src_path = PathBuf::from_str(
                sub_matches.get_one::<String>("src-path")
                .ok_or(mize_err!("no src-path argument"))?
                .as_str()
            )?;
        let config: ItemData = match sub_matches.get_one::<String>("config") {
            None => {
                let mut config = ItemData::new();
                config.set_path("system", "x86_64-linux-gnu")?;
                config
                // TODO: get modName from the only module, which is in this folder... error if there
                // are multiple
                // then set the config arg no longer required
            },
            Some(mut config_str) => {
                let mut tmp = String::new();
                if !config_str.contains("=") {
                    tmp = format!("modName={}", config_str);
                    config_str = &tmp
                };

                let mut config = data_from_string((*config_str).to_owned())?;

                if let Err(_) = config.get_path("system")?.value_string() {
                    config.set_path("system", "x86_64-linux-gnu")?;
                }

                config
            },
        };

        self.add_buildable(name.to_owned(), src_path, config)?;

        self.store_data()?;

        Ok(())
    }

    pub fn cmd_remove_buildable(&mut self, sub_matches: &ArgMatches) -> MizeResult<()> {
        self.load_data()?;

        let name: &String = sub_matches.get_one("name")
            .ok_or(mize_err!("argument name not found"))?;

        self.remove_buildable(name)?;

        self.store_data()?;

        Ok(())
    }


    pub fn run_build(&mut self) -> MizeResult<()> {
        // this is not a clean implementation
        // eventually the dev_shell should be a running mize instance also having the module loaded
        // all sync and state will then be done via mize itself


        // initialize or clear all outputs
        for buildable in &self.data.buildables {
            match self.outputs.get_mut(&buildable.name) {
                Some(val) => {
                    *val = Vec::new();
                },
                None => {
                    let val = Vec::new();
                    self.outputs.insert(buildable.name.clone(), val);
                },
            }
        };


        // set all statuses to "building"
        for buildable in &self.data.buildables {
            match self.state.get_mut(&buildable.name) {
                Some(val) => {
                    *val = "building".to_owned();
                },
                None => {
                    self.state.insert(buildable.name.clone(), "building".to_owned());
                },
            }
        };


        for buildable in &self.data.buildables {

            let system = buildable.config.get_path("system")?.value_string()?;
            let src_path = &buildable.src_path;

            // we redirect all output from the command we spawn to stderr and stdout only gets the
            // pid from the prepended echo $$ command
            // like this we only need to capture output from stderr and only read_line() the pid
            // from stdout
            let command = format!(r#"
                bash -c 'echo $$
                set -e
                export out={}/dist/{system}
                export build_dir={}
                export debugOrRelease=debug
                ( {} )>&2'
            "#, src_path.display(), src_path.display(), buildable.command);

            let cancel_output_thread_one = Arc::new(AtomicBool::new(false));
            let cancel_output_thread_two = cancel_output_thread_one.clone();
            let cancel_output_thread_three = cancel_output_thread_one.clone();

            //println!("command: {}",  command);

            // write bash -c  to stdin of shell
            let mut stdin = self.dev_shells.get(&buildable.name).unwrap().stdin.lock()?;

            //let mut tmp = Vec::new();
            //stdout.buffer().
            stdin.write_all(command.as_bytes());
            drop(stdin);


            let mut stdout = self.dev_shells.get(&buildable.name).unwrap().stdout.lock()?;
            let mut pid_string = String::new();
            println!("pid_string: {}", pid_string);
            stdout.read_line(&mut pid_string)?;
            pid_string.pop();
            let pid: i32 = pid_string.parse()?;

            self.event_tx.send(DevModuleEvent::BuildOutput((buildable.name.clone(), format!("pid of shell running the command: {}\n", pid))));
            let child = &self.dev_shells.get(&buildable.name).unwrap().child;
            self.event_tx.send(DevModuleEvent::BuildOutput((buildable.name.clone(), format!("pid of devShell: {}\n", child.id()))));

            drop(stdout);

            self.event_tx.send(DevModuleEvent::BuildOutput((buildable.name.clone(), "############## NEW BUILD ##############\n".to_owned())))?;

            // thread to read the stderr at the same time
            let stderr_clone = self.dev_shells.get(&buildable.name).unwrap().stderr.clone();
            let event_tx_clone = self.event_tx.clone();
            let name_clone = buildable.name.clone();
            std::thread::spawn(move || {
                let mut buf = String::new();

                let mut stderr = stderr_clone.lock().unwrap();

                while !cancel_output_thread_two.load(std::sync::atomic::Ordering::Acquire) {
                    buf.clear();

                    stderr.read_line(&mut buf);

                    event_tx_clone.send(DevModuleEvent::BuildOutput((name_clone.clone(), buf.clone())));
                }
            });
          
            // spawn thread, which waits for process by id to cancel and then sends an BuildFinished event
            // and tells the output writing thread to exit
            let event_tx_clone = self.event_tx.clone();
            let name_clone = buildable.name.clone();
            std::thread::spawn(move || {
                let mut handle = waitpid_any::WaitHandle::open(pid).unwrap();
                handle.wait().unwrap();
                cancel_output_thread_three.store(true, std::sync::atomic::Ordering::Relaxed);
                event_tx_clone.send(DevModuleEvent::BuildFinished(name_clone));
            });
        }


        Ok(())
    }

    pub fn remove_buildable(&mut self, name: &String) -> MizeResult<()> {

        let index = self.data.buildables.iter().position(|el| &el.name == name)
            .ok_or(mize_err!("Buildable with name '{}' does not exist in current dev env", name))?;

        self.data.buildables.remove(index);

        Ok(())
    }

    pub fn add_buildable(&mut self, name: String, src_path: PathBuf, config: ItemData) -> MizeResult<()> {

        // check if this name already exists
        for buildable in &self.data.buildables {
            if name == buildable.name {
                return Err(mize_err!("A Buildable with the name '{}' already exists in this development environment", name));
            }
        }

        let mod_name = config.get_path("modName")?.value_string()?;
        println!("config: {}", config);
        println!("modName: {}", mod_name);

        let system_str = "x86_64-linux-gnu"; // TODO: get from config or use currentSystem

        let expr = format!(r#"
            let 
                mize = builtins.getFlake "{}";
                mizeBuildPhase = mize.packages.x86_64-linux.mizeFor.{system_str}.modules.{mod_name}.mizeBuildPhase;
                mizeInstallPhase = mize.packages.x86_64-linux.mizeFor.{system_str}.modules.{mod_name}.mizeInstallPhase;
            in mizeBuildPhase + mizeInstallPhase
        "#, self.data.mize_flake);
        std::env::set_var("MIZE_MODULE_NO_REPO", "1");
        std::env::set_var("MIZE_MODULE_NO_EXTERNALS", "1");
        std::env::set_var("MIZE_MODULE_PATH", src_path.as_os_str());
        let command = run_fun!(nix eval --impure --raw --expr $expr)?;

        let buildable = Buildable {
            name: name.to_owned(),
            active: true,
            config,
            src_path,
            command,
        };

        self.data.buildables.push(buildable);

        Ok(())
    }

}

    impl Default for DevModuleData {
        fn default() -> Self {
            DevModuleData {
                last_build: 0,
                buildables: Vec::new(),
                mize_flake: "github:c2vi/mize".to_owned(),
                config: ItemData::new(),
            }
        }
    }



fn handle_line(event_tx: Sender<DevModuleEvent>, instance: &mut Instance, line: String) -> MizeResult<()> {
    let split_tmp = shell_words::split(line.as_str())
        .mize_result_msg("dev module: error spliting the line read from pipe")?;
    let split = split_tmp.iter().map(|v|v.as_str());

    match split.clone().nth(0) {
        // we got some output from the child stdin
        Some("BuildOutput") => {
            let name = split.clone().nth(1).unwrap_or("");
            let output = split.clone().skip(2).collect::<Vec<&str>>().join(" ");
            event_tx.send(DevModuleEvent::BuildOutput((name.to_string(), output.to_string())))?;
        }

        Some("BuildFinished") => {
            let name = split.clone().nth(1).unwrap_or("");
            event_tx.send(DevModuleEvent::BuildFinished(name.to_string()))?;
        }

        Some(_) | None => {
            instance.report_err(mize_err!("dev module: invalid line read from pipe"));
        }
    }

    Ok(())
}
