mod runtime;
mod signals;
mod terminal;

use rinf::{dart_shutdown, write_interface};
use tokio::spawn;
use tokio_with_wasm::alias as tokio;

write_interface!();

#[tokio::main(flavor = "current_thread")]
async fn main() {
    spawn(runtime::run());
    dart_shutdown().await;
}
