// How the phone's movement becomes the covers' tilt: what the orientation sensor reports, turned
// to the screen's axes, less how the phone has been held lately, smoothed. Held still in any
// position, it settles back to nothing, and then stays quiet. motion.svelte.ts feeds it.

/** How the phone has been held lately follows it this slowly, in ms. */
const SETTLE_MS = 1500;
/** Smoothing over the sensor's jitter, in ms. */
const SMOOTH_MS = 100;
/** A turn this far, in degrees, tilts all the way. */
const RANGE = 25;
/** A reading this far from the last (degrees) is the sensor flipping over: start again from it. */
const JUMP = 45;
/** Closer to level than this is level. */
const REST = 0.02;
/** Smaller changes than this aren't passed on. */
const QUIET = 0.005;

export interface Filter {
	/** How the phone has been held lately, in degrees along the screen's axes. */
	base: [number, number] | null;
	value: [number, number];
	/** What was last passed on. */
	sent: [number, number];
}

export const filter = (): Filter => ({ base: null, value: [0, 0], sent: [0, 0] });

/** The sensor's `beta` and `gamma` along the screen's x and y, with the screen turned `angle` degrees. */
export function screenAxes(beta: number, gamma: number, angle: number): [number, number] {
	switch (((angle % 360) + 360) % 360) {
		case 90:
			return [beta, -gamma];
		case 180:
			return [-gamma, -beta];
		case 270:
			return [-beta, gamma];
		default:
			return [gamma, beta];
	}
}

const clamp = (v: number) => Math.max(-1, Math.min(1, v));

/**
 * Takes a reading `dt` ms after the last. Returns the tilt to show, -1…1 along the screen's x and
 * y, or null when it has hardly changed since the last one returned.
 */
export function step(f: Filter, beta: number, gamma: number, angle: number, dt: number): [number, number] | null {
	const reading = screenAxes(beta, gamma, angle);
	const base = f.base;
	if (!base || reading.some((v, i) => Math.abs(v - base[i]) > JUMP)) {
		f.base = [...reading];
	} else {
		const k = 1 - Math.exp(-dt / SETTLE_MS);
		for (const i of [0, 1]) base[i] += (reading[i] - base[i]) * k;
	}
	const k = 1 - Math.exp(-dt / SMOOTH_MS);
	for (const i of [0, 1]) f.value[i] += (clamp((reading[i] - f.base![i]) / RANGE) - f.value[i]) * k;

	const rest = Math.hypot(f.value[0], f.value[1]) < REST;
	const next: [number, number] = rest ? [0, 0] : [f.value[0], f.value[1]];
	const change = Math.max(Math.abs(next[0] - f.sent[0]), Math.abs(next[1] - f.sent[1]));
	if (change === 0 || (!rest && change < QUIET)) return null;
	f.sent = next;
	return next;
}
