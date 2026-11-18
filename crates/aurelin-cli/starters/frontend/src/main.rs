use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    __AURELIN_CRATE__::run().await
}
