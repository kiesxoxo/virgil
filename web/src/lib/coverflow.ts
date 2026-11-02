// The cover flow's geometry, shared by its WebGL renderer and by tapping a cover: how big things
// are, where each cover sits for a scroll position and how the camera sees it. CSS pixels within
// the flow, y down and z towards the viewer, as in CSS transforms.

/** How far the side covers turn towards the middle, in degrees. */
export const ANGLE = 55;
/** Room above the covers. */
export const TOP = 24;
/** The reflection's height, in covers. */
export const REFLECTION = 0.35;
/** Between a cover and its reflection. */
export const GAP = 4;
/** The album's title and Play button under the covers. */
export const CAPTION = 100;
/** How far the whole flow turns with the phone, at most, in degrees. */
export const TURN = 4;

/** The phone's tilt, -1…1 along the screen's x and y (lib/motion.svelte.ts). */
export type Turn = [x: number, y: number];
const FLAT: Turn = [0, 0];

/** The flow's turn for `turn` as CSS `rotateX(…) rotateY(…)` would have it, in radians. */
export function turnAngles([x, y]: Turn): [rotateX: number, rotateY: number] {
	return [(-y * TURN * Math.PI) / 180, (x * TURN * Math.PI) / 180];
}

export interface Layout {
	/** The flow's width. */
	width: number;
	/** The flow's height: the covers and their reflections. */
	height: number;
	/** A cover's edge. */
	cover: number;
	/** Distance between neighbouring covers; the middle one gets `room` more on each side. */
	slot: number;
	room: number;
	reflection: number;
	/** The camera's distance from the page. */
	perspective: number;
}

/** Sizes for a flow `width` wide with `available` height for it and its caption. */
export function layout(width: number, available: number): Layout {
	// As big as fits with the Play button above the bars at the bottom.
	const fit = (available - CAPTION - TOP) / (1 + REFLECTION);
	const cover = Math.round(Math.max(120, Math.min(width * 0.68, fit, 340)));
	const reflection = Math.round(cover * REFLECTION);
	return {
		width,
		height: TOP + cover + reflection,
		cover,
		slot: Math.round(cover * 0.42),
		room: cover * 0.42,
		reflection,
		perspective: cover * 3
	};
}

export interface Placement {
	/** The cover's centre across the flow. */
	x: number;
	/** How far back it is (negative). */
	z: number;
	/** Its turn about the vertical axis, in radians. */
	angle: number;
	/** -1…1: from fully turned left of the middle to fully turned right. */
	d: number;
}

/** Where cover `index` is with the flow scrolled to `position` (in covers). */
export function place(l: Layout, index: number, position: number): Placement {
	const d = Math.max(-1, Math.min(1, index - position));
	return {
		x: l.width / 2 + (index - position) * l.slot + d * l.room,
		z: -Math.abs(d) * l.cover * 0.6,
		angle: (-d * ANGLE * Math.PI) / 180,
		d
	};
}

/** The height of the covers' centres: also where the camera looks from. */
export function middle(l: Layout): number {
	return TOP + l.cover / 2;
}

/**
 * Where the point `(u, v)` of a placed cover (from its centre) lands on the screen, with the
 * whole flow turned by `turn` about the middle cover's centre (as the camera looks straight at
 * it, CSS `perspective(…) rotateX(…) rotateY(…)` on that cover turns it the same way).
 */
export function project(l: Layout, p: Placement, u: number, v: number, turn: Turn = FLAT): [number, number] {
	// From the middle cover's centre.
	let x = p.x + u * Math.cos(p.angle) - l.width / 2;
	let y = v;
	let z = p.z - u * Math.sin(p.angle);
	if (turn[0] || turn[1]) {
		const [ax, ay] = turnAngles(turn);
		[x, z] = [x * Math.cos(ay) + z * Math.sin(ay), z * Math.cos(ay) - x * Math.sin(ay)];
		[y, z] = [y * Math.cos(ax) - z * Math.sin(ax), y * Math.sin(ax) + z * Math.cos(ax)];
	}
	const k = l.perspective / (l.perspective - z);
	return [l.width / 2 + x * k, middle(l) + y * k];
}

/** Covers from `first` up to `last` (exclusive) can be in sight. */
export function visible(l: Layout, position: number, count: number): { first: number; last: number } {
	// Past the middle three, covers are all as far back: they shrink by `k`, at least.
	const k = l.perspective / (l.perspective + l.cover * 0.6);
	const reach = Math.ceil((l.width / 2 + l.cover / 2) / k / l.slot) + 1;
	return {
		first: Math.max(0, Math.floor(position) - reach),
		last: Math.min(count, Math.ceil(position) + reach + 1)
	};
}

/** Indices from `first` to `last`, the furthest from the middle first (the order to draw them in). */
export function backToFront(first: number, last: number, position: number): number[] {
	const indices = Array.from({ length: Math.max(0, last - first) }, (_, i) => first + i);
	return indices.sort((a, b) => Math.abs(b - position) - Math.abs(a - position));
}

/** The cover drawn at `(x, y)` in the flow, turned by `turn`, or -1. */
export function hitTest(l: Layout, position: number, count: number, x: number, y: number, turn: Turn = FLAT): number {
	const { first, last } = visible(l, position, count);
	const h = l.cover / 2;
	for (const index of backToFront(first, last, position).reverse()) {
		const p = place(l, index, position);
		const corners = ([[-h, -h], [h, -h], [h, h], [-h, h]] as const).map(([u, v]) => project(l, p, u, v, turn));
		if (inside(corners, x, y)) return index;
	}
	return -1;
}

/** Whether `(x, y)` is inside the convex polygon `corners`, in either winding. */
function inside(corners: [number, number][], x: number, y: number): boolean {
	let sign = 0;
	for (let i = 0; i < corners.length; i++) {
		const [ax, ay] = corners[i];
		const [bx, by] = corners[(i + 1) % corners.length];
		const cross = (bx - ax) * (y - ay) - (by - ay) * (x - ax);
		if (cross === 0) continue;
		if (sign && Math.sign(cross) !== sign) return false;
		sign = Math.sign(cross);
	}
	return true;
}

/** The dice's spin (in covers and seconds): it pulls back a little, rushes off, and slows to the pick. */
const WIND = 0.3;
const WIND_TIME = 0.18;
const RUSH_TIME = 0.35;
/** Its top speed, covers per second: a blur. */
const SPEED = 45;
/** It slows down over this many covers, crawling past the last few: (1 - t)^EASE. */
const COAST = 30;
const EASE = 3.5;

export interface Spin {
	/** Seconds. */
	duration: number;
	/** The scroll position (in covers) `t` seconds in. */
	at(t: number): number;
}

/**
 * The flow spinning from cover `from` to cover `to`, of `count`. However far apart they are, it
 * takes as long: at top speed, the covers a blur, it jumps to `COAST` covers before the pick,
 * which nobody sees. Where the list is too short for that, it just slows to the pick from the start.
 */
export function spin(from: number, to: number, count: number): Spin {
	const last = count - 1;
	const fits = (p: number) => p >= 0 && p <= last;
	const rush = (SPEED * RUSH_TIME) / 2;
	const toward = Math.sign(to - from) || 1;
	const way = [toward, -toward].find((d) => fits(from + d * rush) && fits(to - d * COAST));
	const ease = (s: number) => 1 - (1 - s) ** EASE;
	if (!way) {
		const distance = Math.abs(to - from);
		const duration = Math.max(1.4, (EASE * distance) / SPEED);
		return { duration, at: (t) => (t >= duration ? to : from + (to - from) * ease(Math.max(0, t) / duration)) };
	}
	// Pulling back only as far as there is room.
	const wind = Math.min(WIND, way > 0 ? from : last - from);
	const coast = (EASE * COAST) / SPEED;
	const duration = WIND_TIME + RUSH_TIME + coast;
	return {
		duration,
		at(t) {
			if (t <= 0) return from;
			if (t < WIND_TIME) return from - (way * wind * (1 - Math.cos((Math.PI * t) / WIND_TIME))) / 2;
			const rushing = t - WIND_TIME;
			if (rushing < RUSH_TIME) return from - way * wind + (way * SPEED * rushing ** 2) / (2 * RUSH_TIME);
			if (t >= duration) return to;
			return to - way * COAST * (1 - ease((rushing - RUSH_TIME) / coast));
		}
	};
}
