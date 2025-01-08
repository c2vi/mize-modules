
// to check if a src changed: find . -not \( -path ./target -prune \) -newermt "2025-01-08 17:26:10"

use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Instant;
use mize::item::ItemData;


use mize::instance;
use mize::MizeResult;
use mize::MizeError;
use mize::Instance;


pub struct MizeDevModule {
}

pub struct Data {
    last_build: Instant,
    srcs: Vec<ModuleToBuild>,
}

pub struct MizeDevModule {
    src_path: PathBuf,
    configs: Vec<ItemData>,
}

impl mize::Module for MizeDevModule {
    fn init(&mut self, instance: &Instance) -> MizeResult<()> {

        Ok(())
    }

    fn exit(&mut self, instance: &Instance) -> MizeResult<()> {

        Ok(())
    }

    fn run_cli(&mut self, instance: &Instance, cmd_line: Vec<OsString>) -> Option<MizeResult<()>> {
        Some(enter_dev_env(self, instance))
    }

}

fn enter_dev_env(dev_module: &mut MizeDevModule, instance: Instance) -> MizeResult<()> {

    Ok(())
}


