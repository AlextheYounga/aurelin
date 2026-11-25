use std::error::Error;

/// Register application seeders here, in the order they should run.
pub async fn run(_database_url: &str) -> Result<(), Box<dyn Error>> {
    println!("No seeders registered.");
    Ok(())
}
