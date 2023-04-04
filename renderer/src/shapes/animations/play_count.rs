/// Describes the looping behavior of an animation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PlayCount {
	/// Play the animation the specified number of times in a row. If zero, the animation does not play at all.
	Count(u64),
	/// Play the animation forever.
	#[default]
	Infinite,
}
