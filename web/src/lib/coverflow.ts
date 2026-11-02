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

/**
 * A cover as the flow draws it: where it is, as `place` has it, and how it lies there. At rest it
 * stands on the floor; the dice's shuffle lifts covers off it, askew, as cards in a pile.
 */
export interface Pose {
	index: number;
	/** Its centre: across the flow, how far up (negative) and how far back (negative). */
	x: number;
	y: number;
	z: number;
	/** Its turn about the vertical axis, and in its own plane (clockwise), in radians. */
	angle: number;
	roll: number;
	/** 0–1: how much of its reflection and shadow shows; less under the card above it in a pile. */
	open: number;
}

/** The covers in sight with the flow scrolled to `position`, the furthest from the middle first. */
export function poses(l: Layout, position: number, count: number): Pose[] {
	const { first, last } = visible(l, position, count);
	return backToFront(first, last, position).map((index) => {
		const { x, z, angle } = place(l, index, position);
		return { index, x, y: 0, z, angle, roll: 0, open: 1 };
	});
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

// The dice's shuffle, in seconds. The flow closes up into a pile either side of the middle cover,
// which steps back to be the deck, the piles clear of it. The piles' cards are thrown onto the
// deck, one side and then the other, the pick last; it lands square on top, and the covers around
// it are dealt out from under it. Only the covers in sight at the start are shuffled: none on the
// way to the pick is fetched just to be glimpsed.
const GATHER = 0.28;
/** The first card thrown, its pile clear of the deck; the rest follow over `RIFFLE`, a beat apart (`BEATS`: at least, at most). */
const RIFFLE_AT = 0.26;
const RIFFLE = 0.8;
const BEATS = [0.045, 0.11] as const;
/** A card in the air; the pick, after a breath. */
const FLIGHT = 0.3;
const PICK_FLIGHT = 0.4;
const BREATH = 0.12;
/** A pile's next card moving up to the top. */
const FEED = 0.12;
const DEAL = 0.55;
/** In covers: where the side piles' top cards are, and between the cards in them; how far back the deck is; how far out at the viewer a card in the air comes; how far off the deck's middle one lands, at most. */
const PILE = 1;
const SPACING = 0.03;
const DECK = 0.12;
const HOP = 0.08;
const OFF = 0.03;
/** In degrees: how askew a card lands on the deck, at most, and turns in the air. */
const ASKEW = 3.5;
const TWIST = 2;

export interface Shuffle {
	/** Seconds. */
	duration: number;
	/** When the pick lands on the deck, and the covers around it start to be dealt out. */
	dealt: number;
	/** When each card before it lands. */
	beats: number[];
	/** The covers `t` seconds in, back to front. */
	at(t: number): Pose[];
}

/** A cover in the shuffle: in its pile, then thrown onto the deck. */
interface Thrown {
	index: number;
	/** Its pile, -1 left or 1 right, and its place in it from the top. */
	side: number;
	k: number;
	/** Across the flow from the middle, at the start. */
	start: number;
	/** When it leaves its pile, and how long it is in the air. */
	leaves: number;
	flight: number;
	/** Where it lands on the deck: off its middle, and askew. */
	x: number;
	roll: number;
}

const smooth = (s: number) => (s <= 0 ? 0 : s >= 1 ? 1 : s * s * (3 - 2 * s));
const lerp = (a: number, b: number, s: number) => a + (b - a) * s;

/** How far a cover askew by `roll` is lifted to stand on its lowest corner. */
const dip = (l: Layout, roll: number) => (l.cover / 2) * (Math.cos(roll) + Math.abs(Math.sin(roll)) - 1);

/** The dice's shuffle from cover `from` in the middle to the pick, `to`, of `count`. */
export function shuffle(l: Layout, from: number, to: number, count: number, random = Math.random): Shuffle {
	const { cover, slot } = l;
	const centre = l.width / 2;
	const turned = (ANGLE * Math.PI) / 180;
	const degree = Math.PI / 180;
	const { first, last } = visible(l, from, count);
	const inSight = to >= first && to < last;
	// Either side of the middle cover, nearest it first; not the pick.
	const pile = (side: number) => {
		const out: number[] = [];
		for (let i = from + side; i >= first && i < last; i += side) if (i !== to) out.push(i);
		return out;
	};
	const piles: Record<number, number[]> = { [-1]: pile(-1), [1]: pile(1) };
	// The pick goes under the bigger pile (under its own, if in sight), to be thrown last.
	const side = inSight ? Math.sign(to - from) : Math.sign(piles[1].length - piles[-1].length) || (random() < 0.5 ? -1 : 1);
	const startOf = (index: number) => place(l, index, from).x - centre;

	// One side and then the other, two from one now and then as a riffle goes; each pile from the top.
	const sides = [-1, 1]
		.flatMap((s) => piles[s].map((_, k) => ({ s, key: (k + (s === side ? 0.25 : 0.75) + (random() - 0.5) * 0.6) / piles[s].length })))
		.sort((a, b) => a.key - b.key);
	const beat = Math.min(BEATS[1], Math.max(BEATS[0], RIFFLE / (sides.length + 1)));
	const taken: Record<number, number> = { [-1]: 0, [1]: 0 };
	const cards: Thrown[] = sides.map(({ s }, i) => {
		const k = taken[s]++;
		const index = piles[s][k];
		// Landing a little short, askew the way it turned in the air.
		return { index, side: s, k, start: startOf(index), leaves: RIFFLE_AT + i * beat, flight: FLIGHT, x: s * random() * OFF * cover, roll: s * (0.3 + 0.7 * random()) * ASKEW * degree };
	});
	const own = piles[side];
	const pick: Thrown = {
		index: to,
		side,
		k: own.length,
		// Out of sight, it starts under the last card of its pile.
		start: inSight ? startOf(to) : own.length ? startOf(own[own.length - 1]) : 0,
		leaves: RIFFLE_AT + sides.length * beat + BREATH,
		flight: PICK_FLIGHT,
		x: 0,
		roll: 0
	};
	cards.push(pick);
	const dealt = pick.leaves + pick.flight;

	/** Across the flow from the middle, before it is thrown: closing up into its pile, which moves up as the cards above it go. */
	const held = (c: Thrown, t: number) => {
		let gone = 0;
		for (const o of cards) if (o.side === c.side && o.k < c.k) gone += smooth((t - o.leaves) / FEED);
		return lerp(c.start, c.side * (PILE + (c.k - gone) * SPACING) * cover, smooth(t / GATHER));
	};
	/** How far along its way to the deck it is: 0 until thrown. */
	const flown = (c: Thrown | undefined, t: number) => (c ? Math.max(0, Math.min(1, (t - c.leaves) / c.flight)) : 0);

	function pose(c: Thrown, t: number): Pose {
		if (t < c.leaves) return { index: c.index, x: centre + held(c, t), y: 0, z: -0.6 * cover, angle: -c.side * turned, roll: 0, open: 1 };
		const u = flown(c, t);
		const e = 1 - (1 - u) ** 3;
		const arc = Math.sin(Math.PI * u);
		// The deck squares up as the pick comes.
		const tidy = 1 - smooth(flown(pick, t));
		const roll = c.roll * tidy * e + c.side * TWIST * degree * arc;
		return {
			index: c.index,
			x: centre + lerp(held(c, c.leaves), c.x * tidy, e),
			y: -dip(l, roll),
			z: lerp(-0.6 * cover, c === pick ? 0 : -DECK * cover, e) + HOP * cover * arc,
			angle: -c.side * turned * (1 - e),
			roll,
			open: 1
		};
	}

	/** The covers around the pick, out from under it to their places in the flow, a little past them and back. */
	function deal(t: number): Pose[] {
		const u = Math.min(1, (t - dealt) / DEAL);
		const out = 1 + 2.2 * (u - 1) ** 3 + 1.2 * (u - 1) ** 2;
		const turn = 1 - (1 - Math.min(1, u / 0.6)) ** 3;
		const { first, last } = visible(l, to, count);
		return backToFront(first, last, to).map((index) => {
			const p = place(l, index, to);
			if (index === to) return { index, x: p.x, y: 0, z: 0, angle: 0, roll: 0, open: 1 };
			const inner = place(l, index - Math.sign(index - to), to);
			return {
				index,
				x: centre + (p.x - centre) * out,
				y: 0,
				z: lerp(-DECK * cover, p.z, turn),
				angle: p.angle * turn,
				roll: 0,
				open: Math.min(1, (Math.abs(p.x - inner.x) * out) / slot)
			};
		});
	}

	return {
		duration: dealt + DEAL,
		dealt,
		beats: cards.slice(0, -1).map((c) => c.leaves + c.flight),
		at(t) {
			if (t >= dealt) return deal(t);
			const poses = new Map(cards.map((c) => [c, pose(c, t)]));
			const inHand = cards.filter((c) => t < c.leaves);
			const thrown = cards.filter((c) => t >= c.leaves);
			// In a pile, as much shows as the card above leaves uncovered (more as it is thrown off);
			// on the deck, until the next card lands on it.
			for (const c of inHand) {
				const above = cards.find((o) => o.side === c.side && o.k === c.k - 1);
				if (above) poses.get(c)!.open = Math.min(1, Math.abs(poses.get(c)!.x - poses.get(above)!.x) / slot);
			}
			const covered = (next: Thrown | undefined) => 1 - smooth((flown(next, t) - 0.5) / 0.5);
			thrown.forEach((c, i) => (poses.get(c)!.open = covered(cards[i + 1])));
			const deck: Pose = { index: from, x: centre, y: 0, z: -DECK * cover * smooth(t / GATHER), angle: 0, roll: 0, open: covered(cards[0]) };
			// The piles behind (their outer cards first), then the deck from the bottom up.
			inHand.sort((a, b) => Math.abs(poses.get(b)!.x - centre) - Math.abs(poses.get(a)!.x - centre) || b.k - a.k);
			return [...inHand.map((c) => poses.get(c)!), deck, ...thrown.map((c) => poses.get(c)!)];
		}
	};
}
