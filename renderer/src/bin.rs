use carton::Carton;
use tokio;

use renderer::Renderer;
use renderer::shaders::ShaderTable;
use renderer::state::State;

#[tokio::main]
async fn main() {
	let mut carton = Carton::read("resources.carton").unwrap();

	let mut renderer = Renderer::new().await;

	// load the compiled shaders from the carton
	let mut shader_table = ShaderTable::new();
	shader_table.load_shader_from_carton("data/hello.frag.spv", &mut carton, renderer.get_device()).unwrap();
	shader_table.load_shader_from_carton("data/hello.vert.spv", &mut carton, renderer.get_device()).unwrap();

	// create the initial render pipeline
	renderer.create_pipeline(&State {
		fragment_shader: shader_table.get_shader("data/hello.frag.spv"),
		vertex_shader: shader_table.get_shader("data/hello.vert.spv"),
	});
}
