// Springs, for things that should move like objects: they head for a target, keep their speed
// when the target changes mid-way, and settle in their own time rather than after a fixed duration.

export interface Spring {
	value: number;
	velocity: number;
	target: number;
}

export const spring = (value: number): Spring => ({ value, velocity: 0, target: value });

/** The longest step taken at once: short enough to stay stable for any frame time. */
const STEP = 1 / 240;

/**
 * Moves `s` on by `dt` seconds. `response` is roughly how long it takes to get there, in seconds;
 * `damping` 1 arrives without overshooting, less overshoots a little.
 */
export function advance(s: Spring, dt: number, response = 0.4, damping = 0.9) {
	const stiffness = ((2 * Math.PI) / response) ** 2;
	const friction = (4 * Math.PI * damping) / response;
	for (let left = dt; left > 1e-9; left -= STEP) {
		const h = Math.min(left, STEP);
		s.velocity += (-stiffness * (s.value - s.target) - friction * s.velocity) * h;
		s.value += s.velocity * h;
	}
}

/** At rest on its target, to within `within` (and moving less than ten times that a second). */
export function settled(s: Spring, within = 0.5): boolean {
	return Math.abs(s.value - s.target) < within && Math.abs(s.velocity) < within * 10;
}
