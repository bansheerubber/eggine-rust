/// The render state stores attributes that describe a render pipeline. The `Renderer` will take the intermediate
/// data structures and translate them into the appropriate `wgpu` render pipeline. Render pipelines are cached by the
/// `Renderer`, and the render state must be hashable to assist caching.
#[derive(Clone, Eq, Hash, PartialEq)]
pub struct State {

}
