use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    __KILN_CRATE__::run().await
}
