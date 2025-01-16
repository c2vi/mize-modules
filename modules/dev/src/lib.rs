
// to check if a src changed: find . -not \( -path ./target -prune \) -newermt "2025-01-08 17:26:10"
#![ allow( warnings ) ]

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


pub mod tui;


#[derive(Debug, Clone)]
pub struct DevModule {
}

#[derive(Serialize, Deserialize)]
pub struct DevModuleData {
    last_build: u32,
    mize_flake: String,
    buildables: Vec<Buildable>,
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
    let new_box: Box<dyn Module + Send + Sync> = Box::new( DevModule {} );

    *empty_module = new_box
}

impl mize::Module for DevModule {
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
                    .required(false)
                )
                .arg(Arg::new("system")
                    .long("system")
                    .short('s')
                    .help("the modName or build-config of the module")
                )
            )
            .subcommand(
                ClapCommand::new("list")
                .aliases(["ls"])
                .about("list all modules/buildables in current dev env")
            )
        ;
        let cmd: Vec<&str> = cmd_line.iter().map(|s|s.to_str().expect("")).collect();
        println!("cmdline: {:?}", cmd);
        let matches = cli.get_matches_from(cmd_line);

        let res = match matches.subcommand() {
            Some(("add", sub_matches)) => cmd_add_buildable(self, instance, sub_matches),
            Some(("list", sub_matches)) => cmd_list_buildable(self, instance, sub_matches),
            Some((cmd, _)) => { return Some(Err(mize_err!("unknown subcommand '{}'", cmd)));},
            None => enter_dev_env(self, instance),
        };

        return Some(res);
    }

}


fn enter_dev_env(dev_module: &mut DevModule, instance: &Instance) -> MizeResult<()> {


    ///////////////// read the mize_dev.json file
    // in the future, this data should be stored in the instance itself
    let data = load_data(instance)?;

    ///////////////// spawn all dev shells for all ModuleToBuild
    let dev_shells: Vec<Command> = Vec::new();



    ///////////////// spawn dev shell for all ModuleToBuild

    ///////////////// run the tui
    tui::run_tui(data, instance);

    Ok(())
}

impl Default for DevModuleData {
    fn default() -> Self {
        DevModuleData {
            last_build: 0,
            buildables: Vec::new(),
            mize_flake: "github:c2vi/mize".to_owned(),
        }
    }
}

pub fn load_data(instance: &Instance) -> MizeResult<DevModuleData> {
    let data_path = PathBuf::from(instance.get("0/config/store_path")?.value_string()?).join("mize_dev_data.json");
    println!("data_path: {}", data_path.display());
    if !data_path.exists() {
        let string = serde_json::to_string(&DevModuleData::default())?;
        fs::write(&data_path, string);
    };
    let data: DevModuleData = serde_json::from_reader(File::open(&data_path)?)
        .map_err(|e| mize_err!("Failed to read mize dev module's data from mize_store_dir/mize_dev_data.json: {e}"))?;

    return Ok(data);

}


pub fn store_data(instance: &Instance, data: DevModuleData) -> MizeResult<()> {
    let data_path = PathBuf::from(instance.get("0/config/store_path")?.value_string()?).join("mize_dev_data.json");

    serde_json::to_writer(File::options().write(true).open(&data_path)?, &data)
        .mize_result_msg("failed to encode DevModuleData into json with serde")?;

    Ok(())
}


pub fn cmd_list_buildable(module: &mut DevModule, instance: &Instance, sub_matches: &ArgMatches) -> MizeResult<()> {
    let mut data = load_data(&instance)?;

    println!("Buildables:");
    for buildable in &data.buildables {
        println!("{} \t\t active:{} \t\t at:{}", buildable.name, buildable.active, buildable.src_path.display())
    }

    store_data(&instance, data)?;

    Ok(())
}


pub fn cmd_add_buildable(module: &mut DevModule, instance: &Instance, sub_matches: &ArgMatches) -> MizeResult<()> {

    let mut data = load_data(&instance)?;

    add_buildable(&mut data, sub_matches)?;

    store_data(&instance, data)?;

    Ok(())
}


pub fn run_build(data: &DevModuleData, instance: &Instance) -> MizeResult<()> {



    Ok(())
}

pub fn add_buildable(data: &mut DevModuleData, sub_matches: &ArgMatches) -> MizeResult<()> {

    let name = sub_matches.get_one::<String>("name")
        .ok_or(mize_err!("no name argument"))?;

    let src_path = PathBuf::from_str(
            sub_matches.get_one::<String>("src-path")
            .ok_or(mize_err!("no src-path argument"))?
            .as_str()
        )?;

    let config: ItemData = match sub_matches.get_one::<String>("config") {
        None => {
            ItemData::new()
            // TODO: get modName from the only module, which is in this folder... error if there
            // are multiple
        },
        Some(mut config_str) => {
            let mut tmp = String::new();
            if !config_str.contains("=") {
                tmp = format!("modName={}", config_str);
                config_str = &tmp
            };

            let config = data_from_string((*config_str).to_owned())?;
            config
        },
    };

    let mod_name = config.get_path("config/modName")?.value_string()?;
    println!("config: {}", config);
    println!("modName: {}", mod_name);

    let system_str = "x86_64-linux-gnu"; // TODO: get from config or use currentSystem

    let expr = format!(r#"
        let 
            mize = builtins.getFlake "{}";
            mizeBuildPhase = mize.packages.x86_64-linux.mizeFor.{system_str}.modules.{mod_name}.mizeBuildPhase;
            mizeInstallPhase = mize.packages.x86_64-linux.mizeFor.{system_str}.modules.{mod_name}.mizeInstallPhase;
        in mizeBuildPhase + mizeInstallPhase
    "#, data.mize_flake);
    std::env::set_var("MIZE_MODULE_NO_REPO", "1");
    std::env::set_var("MIZE_MODULE_NO_EXTERNALS", "1");
    std::env::set_var("MIZE_MODULE_PATH", src_path.to_str().unwrap());
    let command = run_fun!(nix eval --impure --raw --expr $expr)?;

    let buildable = Buildable {
        name: name.to_owned(),
        active: true,
        config,
        src_path,
        command,
    };

    data.buildables.push(buildable);

    Ok(())
}



// currently unused
pub fn spawn_dev_shell() -> MizeResult<()> {
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

