use carton::Carton;
use tokio;

use renderer::{ Memory, Renderer, ShapeBlueprint, };
use renderer::shaders::ShaderTable;
use renderer::state::State;

#[tokio::main]
async fn main() {
	let mut carton = Carton::read("resources.carton").unwrap();

	let event_loop = winit::event_loop::EventLoop::new();
	let mut renderer = Renderer::new(&event_loop).await;

	let mut memory = Memory::new(renderer.get_queue());

	// load the compiled shaders from the carton
	let mut shader_table = ShaderTable::new();
	shader_table.load_shader_from_carton("data/hello.frag.spv", &mut carton, renderer.get_device()).unwrap();
	shader_table.load_shader_from_carton("data/hello.vert.spv", &mut carton, renderer.get_device()).unwrap();

	// create the initial render pipeline
	renderer.create_pipeline(&State {
		fragment_shader: shader_table.get_shader("data/hello.frag.spv").unwrap(),
		vertex_shader: shader_table.get_shader("data/hello.vert.spv").unwrap(),
	});

	ShapeBlueprint::load("data/test.fbx", &mut carton, renderer.get_device(), &mut memory).unwrap();

	// event loop must be created on the main thread
	event_loop.run(move |event, _, control_flow| {
		match event {
			winit::event::Event::RedrawEventsCleared => {
				renderer.get_window().request_redraw();
			},
			winit::event::Event::RedrawRequested(_) => {
				renderer.tick(&mut memory);
			},
			winit::event::Event::WindowEvent {
				event:
					winit::event::WindowEvent::Resized(size)
					| winit::event::WindowEvent::ScaleFactorChanged {
						new_inner_size: &mut size,
						..
					},
				..
			} => {
				renderer.resize(size.width.max(1), size.height.max(1));
			},
			winit::event::Event::WindowEvent {
				event: winit::event::WindowEvent::CloseRequested,
				..
			} => {
				*control_flow = winit::event_loop::ControlFlow::Exit;
			},
			_ => {},
		}
	})
}
