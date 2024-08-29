
use mize::Module;
use mize::Instance;
use mize::MizeResult;

use tracing::info;

static TESTING: &str = "hoooooooooo";

#[no_mangle]
pub struct BlobModule {
    hi: String
}

pub struct MyBox (Box<Box<dyn Module>>);

impl MyBox {
    pub fn new() -> MyBox {
        MyBox ( Box::new(Box::new(BlobModule { hi: "BlobModule MyBox string".to_owned() })))
    }
}

impl Drop for MyBox {
    fn drop(&mut self) {
        println!("would drop MyBox");
    }
}


#[no_mangle]
extern "C" fn get_mize_module_Blob(empty_module: &mut Box<dyn Module + Send + Sync>) -> () {
    let new_box: Box<dyn Module + Send + Sync> = Box::new(BlobModule {hi: "indies BlobModule twoooooooo".to_owned()});

    *empty_module = new_box
}

impl BlobModule {
}

impl Module for BlobModule {
    fn init(&mut self, _instance: &Instance) -> MizeResult<()> {
        println!("BlobModule inittttttttttttttttttttttttttttttt");
        Ok(())
    }

    fn exit(&mut self, _instance: &Instance) -> MizeResult<()> {
        info!("BlobModule exit");
        Ok(())
    }
    
}
