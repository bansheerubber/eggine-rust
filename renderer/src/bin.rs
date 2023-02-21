use carton::Carton;
use renderer::shape;
use renderer::textures;
use renderer::testing::IndirectPass;
use tokio;

use renderer::Boss;

#[tokio::main]
async fn main() {
	let mut carton = Carton::read("resources.carton").unwrap();

	let event_loop = winit::event_loop::EventLoop::new();
	let mut boss = Boss::new(&event_loop).await;

	// create test indirect pass
	let mut test_pass = Box::new(IndirectPass::new(&mut boss, &mut carton));

	// load the first test shape
	let blueprint = shape::Blueprint::load("data/test.fbx", &mut carton, &mut test_pass).unwrap();
	let blueprint = test_pass.add_blueprint(blueprint);

	let shape = shape::Shape::new(blueprint.clone());
	test_pass.add_shape(shape);

	// load the second test shape
	let blueprint = shape::Blueprint::load("data/lizard.fbx", &mut carton, &mut test_pass).unwrap();
	let blueprint = test_pass.add_blueprint(blueprint);

	// load a test texture
	let texture = textures::Texture::load("data/gecc.qoi", &mut carton, &mut test_pass).unwrap();

	for _ in 0..10 {
		let mut shape = shape::Shape::new(blueprint.clone());
		shape.set_texture(Some(texture.clone()));
		test_pass.add_shape(shape);
	}

	// set the boss's passes
	boss.set_passes(vec![test_pass]);

	// event loop must be created on the main thread
	event_loop.run(move |event, _, control_flow| {
		match event {
			winit::event::Event::RedrawEventsCleared => {
				boss.get_context().window.request_redraw();
			},
			winit::event::Event::RedrawRequested(_) => {
				boss.tick();
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
				boss.resize(size.width.max(1), size.height.max(1));
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
