use tokio;

use renderer::Renderer;

#[tokio::main]
async fn main() {
	let renderer = Renderer::new().await;
}
