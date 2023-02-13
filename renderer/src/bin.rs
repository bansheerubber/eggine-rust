use carton::Carton;
use renderer::shape::{ ShapeBuffer, Shape, };
use tokio;

use renderer::{ Boss, Memory, ShapeBlueprint, };
use renderer::shaders::ShaderTable;
use renderer::state::State;

#[tokio::main]
async fn main() {
	let mut carton = Carton::read("resources.carton").unwrap();

	let event_loop = winit::event_loop::EventLoop::new();
	let mut renderer = Boss::new(&event_loop).await;

	let mut memory = Memory::new(renderer.get_context());
	renderer.initialize_buffers(&mut memory);

	// load the compiled shaders from the carton
	let mut shader_table = ShaderTable::new(renderer.get_context());
	shader_table.load_shader_from_carton("data/hello.frag.spv", &mut carton).unwrap();
	shader_table.load_shader_from_carton("data/hello.vert.spv", &mut carton).unwrap();

	// create the initial render pipeline
	renderer.create_pipeline(&State {
		fragment_shader: shader_table.get_shader("data/hello.frag.spv").unwrap(),
		vertex_shader: shader_table.get_shader("data/hello.vert.spv").unwrap(),
	});

	// create shape buffer used for indirect rendering
	let mut buffer = ShapeBuffer::new(&mut memory);

	let blueprint = ShapeBlueprint::load("data/test.fbx", &mut carton, &mut memory, &mut buffer).unwrap();
	let shape = Shape::new(blueprint.clone());

	let mut buffer = Vec::new();
	shape.write_indirect_buffer(&mut buffer);

	// event loop must be created on the main thread
	event_loop.run(move |event, _, control_flow| {
		match event {
			winit::event::Event::RedrawEventsCleared => {
				renderer.get_context().window.request_redraw();
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
