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

/** Where the point `(u, v)` of a placed cover (from its centre) lands on the screen. */
export function project(l: Layout, p: Placement, u: number, v: number): [number, number] {
	const x = p.x + u * Math.cos(p.angle);
	const z = p.z - u * Math.sin(p.angle);
	const k = l.perspective / (l.perspective - z);
	return [l.width / 2 + (x - l.width / 2) * k, middle(l) + v * k];
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

/** The cover drawn at `(x, y)` in the flow, or -1. */
export function hitTest(l: Layout, position: number, count: number, x: number, y: number): number {
	const { first, last } = visible(l, position, count);
	const h = l.cover / 2;
	for (const index of backToFront(first, last, position).reverse()) {
		const p = place(l, index, position);
		const corners = [project(l, p, -h, -h), project(l, p, h, -h), project(l, p, h, h), project(l, p, -h, h)];
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
