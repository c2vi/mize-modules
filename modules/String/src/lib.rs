
use mize::Module;
use mize::Instance;
use mize::MizeResult;

use tracing::info;

static TESTING: &str = "hoooooooooo";

#[no_mangle]
pub struct StringModule {
    hi: String
}

pub struct MyBox (Box<Box<dyn Module>>);

impl MyBox {
    pub fn new() -> MyBox {
        MyBox ( Box::new(Box::new(StringModule { hi: "StringModule MyBox string".to_owned() })))
    }
}

impl Drop for MyBox {
    fn drop(&mut self) {
        println!("would drop MyBox");
    }
}


#[no_mangle]
extern "C" fn get_mize_module_String(empty_module: &mut Box<dyn Module + Send + Sync>) -> () {
    let new_box: Box<dyn Module + Send + Sync> = Box::new(StringModule {hi: "indies StringModule twoooooooo".to_owned()});

    *empty_module = new_box
}

impl StringModule {
}

impl Module for StringModule {
    fn init(&mut self, _instance: &Instance) -> MizeResult<()> {
        println!("StringModule inittttttttttttttttttttttttttttttt");
        Ok(())
    }

    fn exit(&mut self, _instance: &Instance) -> MizeResult<()> {
        info!("StringModule exit");
        Ok(())
    }
    
}
