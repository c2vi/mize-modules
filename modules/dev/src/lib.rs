
// to check if a src changed: find . -not \( -path ./target -prune \) -newermt "2025-01-08 17:26:10"
#![ allow( warnings ) ]

use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::fs::File;
use std::io::stdout;
use std::io::Stdin;
use std::path::PathBuf;
use std::process::Child;
use std::process::Command;
use clap::Command as ClapCommand;
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


pub struct DevModule {
    dev_shells: HashMap<String, Child>,
    data: DevModuleData,
    instance: Instance,
    tui_state: Option<TuiState>,
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
        DevModule {
            data: DevModuleData::default(),
            dev_shells: HashMap::new(),
            instance,
            tui_state: None,
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
        ;
        let cmd: Vec<&str> = cmd_line.iter().map(|s|s.to_str().expect("")).collect();
        println!("cmdline: {:?}", cmd);
        let matches = cli.get_matches_from(cmd_line);

        match matches.subcommand() {
            Some(("add", sub_matches)) => self.cmd_add_buildable(sub_matches)?,
            Some(("remove", sub_matches)) => self.cmd_remove_buildable(sub_matches)?,
            Some(("list", sub_matches)) => self.cmd_list_buildable(sub_matches)?,

            Some(("shell", sub_matches)) => self.run_shell()?,
            Some((cmd, _)) => { return Err(mize_err!("unknown subcommand '{}'", cmd));},
            None => self.run_tui()?,
        };

        Ok(())
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


    fn tui_state(&mut self) -> &mut TuiState {

        Tui::init_state(self);

        return self.tui_state.as_mut().unwrap();
    }


    fn run_tui(&mut self) -> MizeResult<()> {

        let mut tui = tui::Tui::new(self)?;

        tui.run()?;
        
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

            //let dev_shell_path = format!("{}#packages.{}.mizeFor.{}.modules.{}.devShell", data.mize_flake, host_system, target_system, mod_name);
            let dev_shell_path = format!("{}#packages.{}.mizeFor.{}.modules.{}.devShell", self.data.mize_flake, host_system, target_system, mod_name);
            println!("dev_shell_path: {}", dev_shell_path);

            let mut args = vec!["develop", "--impure", dev_shell_path.as_str()];

            // add --override-input arg, when we have a config/override_nixpkgs set
            if let Ok(nixpkgs_path) = self.data.config.get_path("override_nixpkgs")?.value_string() {
                let arg = format!("--override-input nixpkgs {}", nixpkgs_path);
                args.push(arg.as_str());
            }

            let child = Command::new("nix")
                .arg("develop")
                .arg("--impure")
                .arg(dev_shell_path)
                .env("MIZE_MODULE_NO_REPO", "1")
                .env("MIZE_MODULE_NO_EXTERNALS", "1")
                .env("MIZE_MODULE_PATH", &buildable.src_path)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?
            ;
            //let output = child.wait_with_output()?;
            self.dev_shells.insert(buildable.name.clone(), child);
        }

        Ok(())
    }

    pub fn load_data(&mut self) -> MizeResult<()> {
        let data_path = PathBuf::from(self.instance.get("0/config/store_path")?.value_string()?).join("mize_dev_data.json");
        println!("data_path: {}", data_path.display());

        if !data_path.exists() {
            let string = serde_json::to_string(&DevModuleData::default())?;
            fs::write(&data_path, string);
        };
        self.data = serde_json::from_reader(File::open(&data_path)?)
            .map_err(|e| mize_err!("Failed to read mize dev module's data from mize_store_dir/mize_dev_data.json: {e}"))?;

        Ok(())

    }


    pub fn store_data(&self) -> MizeResult<()> {
        let data_path = PathBuf::from(self.instance.get("0/config/store_path")?.value_string()?).join("mize_dev_data.json");

        serde_json::to_writer_pretty(File::options().write(true).open(&data_path)?, &self.data)
            .mize_result_msg("failed to encode DevModuleData into json with serde")?;

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

        println!("hello world");
        // write bash -c ''; huh.... what to do here???????


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



