#![allow(dead_code, unused_imports)]

use carton::Carton;
use rand::Rng;
use renderer::testing::depth_visualizer::DepthVisualizer;
use renderer::{ memory_subsystem, shapes, Pass, };
use renderer::testing::indirect_pass::IndirectPass;
use std::time::Instant;
use tokio;

// use jemallocator::Jemalloc;

// #[global_allocator]
// static GLOBAL: Jemalloc = Jemalloc;

use renderer::Boss;

#[tokio::main]
async fn main() {
	// let mut carton = Carton::read("resources.carton").unwrap();
	// let mut pager = memory_subsystem::textures::Pager::new(20, 2048);
	// let texture = pager.load_qoi("data/none.qoi", &mut carton).unwrap();

	// let now = Instant::now();
	// let mut count = 1;
	// while pager.allocate_texture(&texture).is_some() {
	// 	count += 1;
	// }

	// let elapsed = now.elapsed();
	// println!("{:.2?} to allocate {} textures ({:.2?} per texture)", elapsed, count, elapsed / count);

	let mut carton = Carton::read("resources.carton").unwrap();

	let event_loop = winit::event_loop::EventLoop::new();
	let mut boss = Boss::new(&event_loop).await;

	// create test indirect pass
	let mut test_pass = IndirectPass::new(&mut boss, &mut carton);

	// load the first test shape
	// let now = Instant::now();

	// let blueprint = {
	// 	let blueprint = shapes::blueprint::Blueprint::load("data/pig.glb", &mut carton, &mut test_pass, boss.get_memory()).unwrap();
	// 	let blueprint = test_pass.add_blueprint(blueprint);

	// 	blueprint
	// };

	// let elapsed = now.elapsed();
	// println!("{:.2?} to load the pig", elapsed);

	// let shape = shapes::Shape::new(blueprint.clone());
	// test_pass.add_shape(shape);

	// let blueprint = shape::blueprint2::Blueprint::load("data/cube.glb", &mut carton, &mut test_pass, boss.get_memory()).unwrap();
	// let blueprint = test_pass.add_blueprint(blueprint);

	// let shape = shape::Shape::new(blueprint.clone());
	// test_pass.add_shape(shape);

	// load the second test shape
	let blueprint = shapes::blueprint::Blueprint::load("data/lizard.glb", &mut carton, &mut test_pass, boss.get_memory()).unwrap();
	let blueprint = test_pass.add_blueprint(blueprint);

	let shape = shapes::Shape::new(blueprint.clone());
	let shape = test_pass.add_shape(shape);

	{
		let mut egg = shape.borrow_mut();

		egg.play_animation(shapes::animations::Context::new(
			"walk",
			shapes::animations::Blending {
				priority: 0,
				weight: 1.0,
			},
			shapes::animations::PlayCount::Count(1),
			1.0
		));

		// egg.get_animation_mut(id).unwrap().pause();
	}

	// {
	// 	shape.borrow_mut().play_animation(shapes::AnimationContext {
	// 		blending: shapes::AnimationBlending {
	// 			priority: 0,
	// 			weight: 1.0,
	// 		},
	// 		looping_behavior: shapes::AnimationIteration::Infinite,
	// 		name: String::from("walk"),
	// 		timescale: 1.0,
	// 		..shapes::AnimationContext::default()
	// 	});
	// }

	// {
	// 	shape.borrow_mut().play_animation(shapes::AnimationContext {
	// 		blending: shapes::AnimationBlending {
	// 			priority: 0,
	// 			weight: 1.0,
	// 		},
	// 		looping_behavior: shapes::AnimationIteration::Infinite,
	// 		name: String::from("walk"),
	// 		timescale: 1.0,
	// 		..shapes::AnimationContext::default()
	// 	});
	// }


	// {
	// 	shape.borrow_mut().play_animation(shapes::AnimationContext {
	// 		blending: shapes::AnimationBlending {
	// 			priority: 0,
	// 			weight: 1.0,
	// 		},
	// 		looping_behavior: shapes::AnimationIteration::Infinite,
	// 		name: String::from("walk"),
	// 		timescale: 1.0,
	// 		..shapes::AnimationContext::default()
	// 	});
	// }

	// {
	// 	shape.borrow_mut().play_animation(shapes::AnimationContext {
	// 		blending: shapes::AnimationBlending {
	// 			priority: 0,
	// 			weight: 1.0,
	// 		},
	// 		looping_behavior: shapes::AnimationIteration::Infinite,
	// 		name: String::from("walk"),
	// 		timescale: 1.0,
	// 		..shapes::AnimationContext::default()
	// 	});
	// }

	// {
	// 	shape.borrow_mut().play_animation(shapes::AnimationContext {
	// 		blending: shapes::AnimationBlending {
	// 			priority: 0,
	// 			weight: 1.0,
	// 		},
	// 		looping_behavior: shapes::AnimationIteration::Infinite,
	// 		name: String::from("walk"),
	// 		timescale: 1.0,
	// 		..shapes::AnimationContext::default()
	// 	});
	// }

	let mut rng = rand::thread_rng();

	for _ in 0..500 {
		let mut shape = shapes::Shape::new(blueprint.clone());
		shape.set_position(glam::Vec3::new(rng.gen::<f32>() * 30.0, rng.gen::<f32>() * 30.0, 0.0));

		test_pass.add_shape(shape);
	}

	// create test depth visualizer
	let mut depth_visualizer = DepthVisualizer::new(&mut boss, &mut carton);
	depth_visualizer.disable();

	let depth_pyramid = test_pass.get_depth_pyramid();
	depth_visualizer.set_depth_pyramid(Some(depth_pyramid));

	boss.set_passes(vec![test_pass, depth_visualizer]);

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
				event: winit::event::WindowEvent::KeyboardInput {
					input: winit::event::KeyboardInput {
						state: winit::event::ElementState::Pressed,
						virtual_keycode,
						..
					},
					..
				},
				..
			} => {
				match virtual_keycode {
					Some(winit::event::VirtualKeyCode::P) => { // toggle depth pyramid debug view
						let pass = boss.get_pass_mut(1).unwrap();
						if pass.is_enabled() {
							pass.disable();
						} else {
							pass.enable();
						}
					},
					Some(winit::event::VirtualKeyCode::M) => { // print memory
						let memory = boss.get_memory();
						let memory = memory.read().unwrap();
						println!("{}", memory);
					},
					_ => {},
				}
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
