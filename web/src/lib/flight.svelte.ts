// Covers that fly from one place to another as the page changes: from the grid, a list or the cover
// flow to the album's page and back, from the mini player to Now Playing and back. The cover lifts
// off the page as an image over it (lib/flyers.ts), flies on springs while the next page comes in,
// leaning into the way it goes, and lands where that page shows it, which then shows it again.
// Where it can't fly (no image yet, Reduce Motion), the page transition morphs it instead. Only that cover gets a view-transition-name: every named element
// is lifted out of the page and animated on its own, so naming all covers would flatten the cover
// flow and make phones stutter.

import { tick } from 'svelte';
import { advance, settled, spring, type Spring } from './spring.ts';
import { flyers, type FlyerCard } from './flyers.ts';
import { stage, type Card } from './stage-gl.ts';

/** A cover on the page that can fly: `album:<id>` for an album's, `now` for what's playing. */
interface Slot {
	el: HTMLElement;
	/** Its artwork key, or a full image URL (radio). */
	art: () => string | null | undefined;
}

/** Where a cover is on the screen (its centre), and how it looks there. */
interface Pose {
	x: number;
	y: number;
	size: number;
	radius: number;
	shadow: number;
	ring: number;
	reflection: number;
	opacity: number;
}

const PARTS = ['x', 'y', 'size', 'radius', 'shadow', 'ring', 'reflection', 'opacity'] as const;
/** Close enough to land: half a pixel, or a hundredth of the way for the parts that run 0–1. */
const WITHIN: Record<(typeof PARTS)[number], number> = {
	x: 0.5,
	y: 0.5,
	size: 0.5,
	radius: 0.5,
	reflection: 0.5,
	shadow: 0.01,
	ring: 0.01,
	opacity: 0.01
};
/** Roughly how long a flight takes, in seconds, and how much it may overshoot (1: not at all). */
const RESPONSE = 0.34;
const DAMPING = 0.9;
/** At most this far a cover leans into its flight, in radians (6°), at this speed (px/s). */
const LEAN = (6 * Math.PI) / 180;
const LEAN_SPEED = 2500;
/** Lifted: it grows a little and its shadow deepens, most at this speed. */
const LIFT_SPEED = 1500;
/** A cover not given anywhere to land by then fades where it is. */
const STRANDED_MS = 2000;

const clamp = (v: number, lo = -1, hi = 1) => Math.max(lo, Math.min(hi, v));

/** Where a card is and how it looks, as the flow draws its covers. */
export type Place = Omit<Card, 'art' | 'opacity' | 'light'> & {
	/** Seen through the cover flow's camera (turned, set back), not straight on. */
	flow?: boolean;
};

/** A cover in sight in the flow, which draws its covers itself rather than as images on the page. */
export interface Placed {
	key: string;
	art: string;
	place: Place;
}

/** A cover in sight: its artwork, where it is, and on the page (not in the flow), its element. */
interface Sighted {
	key: string;
	art: string;
	place: Place;
	/** The image to fly with, if it has one in sight; on the page, that image itself. */
	src: string | null;
	img?: HTMLImageElement;
	el?: HTMLElement;
}

/** A cover flying from one view to the next, or (with nowhere to go) fading where it was. */
interface Mover {
	key: string;
	src: string;
	/** Where it was, as the old view had it. */
	origin: Place;
	/** Where it goes, as the new view has it now: followed on every frame, as the page may still move. */
	goal: Place | null;
	/** The cover it lands on in the new view, when the page shows it (not the flow). */
	el?: HTMLElement;
	/** Both ends seen through one camera, for mixing. */
	from: Place;
	to: Place | null;
	/** 0 where it was … 1 where it goes. */
	t: Spring;
	/** Seconds before it sets off: the covers nearest the middle go first. */
	wait: number;
}

/** Flying from one view to the next. */
const SWITCH_RESPONSE = 0.4;
const SWITCH_DAMPING = 0.86;
/** At most this many covers fly; the rest of the new view fades in. */
const MOVERS = 24;
/** The flyer of a cover flying from one page to the next. */
const FLIGHT = 'flight';

class Flight {
	/** The slot whose cover is in the air: its covers on the page stay hidden. */
	key = $state<string | null>(null);
	/** The slot a page transition morphs instead: its covers are named on both pages. */
	morphing = $state<string | null>(null);

	#slots = new Map<string, Set<Slot>>();
	/** The flying cover's image, its width in pixels, and its size unscaled (its largest on the way). */
	#src = '';
	#width = 0;
	#base = 0;
	/** The covers in the air as last shown. */
	#shown: FlyerCard[] = [];
	#springs: Record<(typeof PARTS)[number], Spring> | null = null;
	/** Where it is going is known (until then it waits where it lifted off). */
	#landing = false;
	/**
	 * The cover it lands on, and how it was when the flight set off: followed on every frame, as the
	 * page may still move under it (content coming in above, its scroll put back).
	 */
	#target: { el: HTMLElement; pose: Pose } | null = null;
	#raf = 0;
	#last = 0;
	#flights = 0;
	#stranded: ReturnType<typeof setTimeout> | undefined;
	/** Slots whose covers are in the air between views: hidden on the page. */
	between = $state.raw<ReadonlySet<string>>(new Set());
	/** Slots whose covers on the page fade in with the new view, rather than fly there. */
	appearing = $state.raw<ReadonlySet<string>>(new Set());
	/** Covers in the air between views, flying or fading where they were: the flow leaves them out. */
	#air = new Set<string>();
	/** Those covers, and those fading where they were. */
	#movers: Mover[] = [];
	#switching = false;
	#switched = 0;
	/** The cover flow's covers in sight, while it is on the page. */
	flow: (() => Placed[]) | null = null;
	#still = typeof window === 'undefined' ? null : matchMedia('(prefers-reduced-motion: reduce)');

	/** Makes `el` a place slot `key`'s cover can fly from or land on, until the returned function is called. */
	register(key: string, el: HTMLElement, art: () => string | null | undefined): () => void {
		const slot = { el, art };
		let slots = this.#slots.get(key);
		if (!slots) this.#slots.set(key, (slots = new Set()));
		slots.add(slot);
		return () => {
			slots.delete(slot);
			if (!slots.size && this.#slots.get(key) === slots) this.#slots.delete(key);
		};
	}

	/** The view-transition-name for slot `key`'s cover: only while a page transition morphs it. */
	name(key: string): string | undefined {
		return this.morphing === key ? `cover-${key.replace(/[^\w-]/g, '-')}` : undefined;
	}

	/** Whether slot `key`'s covers on the page are hidden: their cover is in the air. */
	hides(key: string): boolean {
		return this.key === key || this.between.has(key);
	}

	/** Whether the flow leaves slot `key`'s cover out: the stage draws it in the air. */
	airborne(key: string): boolean {
		return this.key === key || this.#air.has(key);
	}

	/** The album whose cover in sight is nearest the middle of the screen, if any. */
	nearest(): string | null {
		let found: string | null = null;
		let best = Infinity;
		for (const key of this.#slots.keys()) {
			const slot = key.startsWith('album:') && this.#find(key);
			if (!slot) continue;
			const r = slot.el.getBoundingClientRect();
			const d = Math.hypot(r.left + r.width / 2 - innerWidth / 2, r.top + r.height / 2 - innerHeight / 2);
			if (d < best) [found, best] = [key.slice('album:'.length), d];
		}
		return found;
	}

	/**
	 * Changes the view (`change`): the album covers in sight fly from where they were to where the
	 * new view shows them, turning into (or out of) the cover flow; those with nowhere to go fade
	 * where they were. Returns once the new view is in place, with the covers on their way.
	 */
	async switch(change: () => void) {
		// Those with an image to fly with.
		const before = new Map(this.#still?.matches ? [] : [...this.#inSight()].filter(([, sighted]) => sighted.src));
		if (!before.size) return change();
		this.#finish();
		this.#endSwitch();
		const switched = ++this.#switched;
		this.#switching = true;
		// Hidden from the start in the new view too, until they land there.
		this.between = new Set(before.keys());
		this.#air = new Set(before.keys());
		this.#movers = [...before.values()].map(({ key, src, place }) => ({ key, src: src!, origin: place, goal: null, from: place, to: null, t: spring(0), wait: 0 }));
		this.#drawMovers(0);
		// The flow leaves them out from now on, on the same frame.
		stage().draw();
		// The rest of the page cross-fades, under the stage (live through it: app.css).
		if (document.startViewTransition) {
			await tick();
			await document.startViewTransition(async () => {
				change();
				await tick();
			}).updateCallbackDone;
		} else {
			change();
		}
		// The new view laid out (it measures itself a frame after it is made).
		for (let i = 0; i < 2; i++) {
			await tick();
			await new Promise(requestAnimationFrame);
		}
		if (switched !== this.#switched) return;

		// Into the flow, or onto a cover on the page with its image in: not onto a placeholder.
		const after = new Map([...this.#inSight()].filter(([, sighted]) => sighted.place.flow || sighted.src));
		const middle = (p: Place) => Math.hypot(p.eye[0] + p.centre[0] - innerWidth / 2, p.eye[1] + p.centre[1] - innerHeight / 2);
		let flying = this.#movers.filter((m) => after.has(m.key));
		flying.sort((a, b) => middle(after.get(a.key)!.place) - middle(after.get(b.key)!.place));
		flying = flying.slice(0, MOVERS);
		flying.forEach((m, rank) => {
			const { place, el } = after.get(m.key)!;
			[m.goal, m.el] = [place, el];
			aim(m);
			m.wait = Math.min(0.2, rank * 0.012);
			// Into the flow, which draws it from there: with the page's image, not a placeholder.
			const { art, img } = before.get(m.key)!;
			if (place.flow && img) stage().adopt(art, img);
		});
		const going = new Set(flying);
		// Those with nowhere to go fade where they were, beneath those flying; the furthest back are
		// drawn first.
		const left = this.#movers.filter((m) => !going.has(m));
		const depth = (m: Mover) => Math.min(m.from.centre[2], m.to!.centre[2]);
		this.#movers = [...left, ...flying.sort((a, b) => depth(a) - depth(b))];
		for (const m of this.#movers) m.t.target = 1;
		// The flow leaves out those fading where they were in it, as well as those flying.
		this.#air = new Set(this.#movers.filter((m) => going.has(m) || m.from.flow).map((m) => m.key));
		stage().draw();
		// The rest fade in where the new view has them, rather than pop up there.
		this.between = new Set(flying.map((m) => m.key));
		this.appearing = new Set([...before.keys()].filter((key) => !this.between.has(key)));
		this.#last = 0;
		if (!this.#raf) this.#raf = requestAnimationFrame(this.#switchFrame);
	}

	/**
	 * The album covers in sight: on the page, and in the flow. Only those clear of the bars over the
	 * page: in the air they are above them, so one beneath a bar would be drawn over it.
	 */
	#inSight(): Map<string, Sighted> {
		const [top, bottom] = open();
		const clear = (y: number, size: number) => y - size / 2 > top - 1 && y + size / 2 < bottom + 1;
		const found = new Map<string, Sighted>();
		for (const key of this.#slots.keys()) {
			const slot = key.startsWith('album:') && this.#find(key);
			const art = slot && slot.art();
			// The flow's middle cover is one of its own.
			if (!slot || !art || slot.el.closest('[data-reflection]')) continue;
			const pose = poseOf(slot.el);
			const img = best(slot.el);
			if (clear(pose.y, pose.size)) found.set(key, { key, art, place: flat(pose), src: img?.currentSrc || null, img, el: slot.el });
		}
		for (const { key, art, place } of this.flow?.() ?? []) {
			if (clear(place.eye[1] + place.centre[1], place.size)) found.set(key, { key, art, place, src: stage().source(art) });
		}
		return found;
	}

	#switchFrame = (time: number) => {
		this.#raf = 0;
		if (!this.#switching) return;
		const dt = this.#last ? Math.min(0.05, (time - this.#last) / 1000) : 0;
		this.#last = time;
		this.#followMovers();
		let moving = false;
		for (const m of this.#movers) {
			if (m.wait > 0) {
				m.wait -= dt;
				moving = true;
				continue;
			}
			advance(m.t, dt, SWITCH_RESPONSE, SWITCH_DAMPING);
			if (!settled(m.t, 0.002)) moving = true;
		}
		if (moving) this.#raf = requestAnimationFrame(this.#switchFrame);
		this.#drawMovers();
		if (!moving) this.#endSwitch();
	};

	/**
	 * Aims the covers flying between views at where the new view has them now: it may still move
	 * (the page's scroll put back as it got shorter, which Safari on iOS can do a moment late).
	 */
	#followMovers() {
		const flow = new Map(this.flow?.().map((placed) => [placed.key, placed.place]));
		for (const m of this.#movers) {
			if (!m.goal) continue;
			if (m.goal.flow) {
				m.goal = flow.get(m.key) ?? m.goal;
			} else if (m.el?.isConnected) {
				const r = m.el.getBoundingClientRect();
				if (r.width) m.goal = { ...m.goal, eye: [r.left + r.width / 2, r.top + r.height / 2], perspective: r.width * 3, size: r.width };
			}
			aim(m);
		}
	}

	#drawMovers(at?: number) {
		this.#show(
			this.#movers.map((m) => {
				const t = at ?? m.t.value;
				const { flow: _, ...place } = m.to ? mix(m.from, m.to, t) : m.from;
				const base = Math.max(m.origin.size, m.goal?.size ?? 0);
				return { ...place, key: m.key, src: m.src, base, opacity: m.to ? 1 : clamp(1 - t, 0, 1) };
			})
		);
	}

	#show(cards: FlyerCard[]) {
		this.#shown = cards;
		flyers.set(cards);
	}

	/**
	 * The covers in the air as only their covers, for their last frame: over the covers just shown
	 * again on the page (or in the flow), which have their own shadow and reflection. Twice over,
	 * they would darken for a frame. Gone the next, if `still` holds then.
	 */
	#touchDown(still: () => boolean) {
		this.#show(this.#shown.map((card) => ({ ...card, shadow: 0, reflection: 0, ring: 0 })));
		requestAnimationFrame(() => {
			if (still()) this.#show([]);
		});
	}

	/** Every cover between views lands at once: the page and the flow show them again, and they go a frame later. */
	#endSwitch() {
		if (!this.#switching) return;
		cancelAnimationFrame(this.#raf);
		this.#raf = 0;
		this.#last = 0;
		this.#switching = false;
		this.#movers = [];
		this.between = new Set();
		this.appearing = new Set();
		this.#air = new Set();
		stage().draw();
		const switched = ++this.#switched;
		this.#touchDown(() => switched === this.#switched && !this.#switching && !this.#springs);
	}

	/** The centre of slot `key`'s cover on the screen, if it is in sight. */
	centre(key: string): { x: number; y: number } | null {
		const slot = this.#find(key);
		if (!slot) return null;
		const r = slot.el.getBoundingClientRect();
		return { x: r.left + r.width / 2, y: r.top + r.height / 2 };
	}

	/**
	 * Lifts slot `key`'s cover off the page onto the stage, where it waits for `land`. False when
	 * it can't fly: then the page transition should morph it.
	 */
	lift(key: string): boolean {
		if (this.#still?.matches) return false;
		// Already in the air (back before it landed): it turns round from where it is.
		if (this.#springs && this.key === key) {
			this.#landing = false;
			this.#strand();
			return true;
		}
		this.#finish();
		this.#endSwitch();
		const from = this.#find(key);
		const art = from?.art();
		if (!from || !art) return false;
		// The page's image, or (the flow's middle cover, still loading its own) the flow's.
		const img = best(from.el);
		const src = img?.currentSrc || stage().source(art);
		if (!src) return false;

		const pose = poseOf(from.el);
		this.#springs = Object.fromEntries(PARTS.map((part) => [part, spring(pose[part])])) as Record<(typeof PARTS)[number], Spring>;
		[this.#src, this.#width, this.#base] = [src, img?.naturalWidth ?? 0, pose.size];
		this.#landing = false;
		this.#target = null;
		this.#flights++;
		this.key = key;
		flyers.clip(null);
		this.#strand();
		// In the air now, before the page hides its cover; and the flow leaves it out, before the
		// page transition takes its picture.
		this.#frame(performance.now());
		stage().draw();
		return true;
	}

	/** Flies the lifted cover to where slot `key`'s cover is now, or fades it where it is if there is none. */
	land(key: string) {
		const springs = this.#springs;
		if (!springs || this.key !== key) return;
		clearTimeout(this.#stranded);
		const to = this.#find(key);
		if (to) {
			const pose = poseOf(to.el);
			for (const part of PARTS) springs[part].target = pose[part];
			this.#target = { el: to.el, pose };
			this.#base = Math.max(this.#base, pose.size);
			// Landing beneath the bars over the page, it goes under them all the way, rather than over
			// them until it lands. Landing in a bar (the mini player), it stays above them all.
			flyers.clip(to.el.closest('[data-bar]') ? null : open());
		} else {
			springs.opacity.target = 0;
		}
		this.#landing = true;
		this.#fly();
	}

	/**
	 * Starts the springs from here. The clock starts again too: the page stood still meanwhile
	 * (while a view transition took its pictures), and that time would make the cover leap.
	 */
	#fly() {
		if (this.#raf) return;
		this.#last = 0;
		this.#raf = requestAnimationFrame(this.#frame);
	}

	#strand() {
		clearTimeout(this.#stranded);
		const key = this.key;
		this.#stranded = setTimeout(() => {
			if (this.#springs && !this.#landing && key) {
				this.#springs.opacity.target = 0;
				this.#landing = true;
				this.#fly();
			}
		}, STRANDED_MS);
	}

	#frame = (time: number) => {
		this.#raf = 0;
		const springs = this.#springs;
		if (!springs) return;
		const dt = this.#last ? Math.min(0.05, (time - this.#last) / 1000) : 0;
		this.#last = time;
		if (this.#landing) {
			this.#follow(springs);
			for (const part of PARTS) advance(springs[part], dt, RESPONSE, DAMPING);
		}

		const still = !!this.#still?.matches;
		const { x, y } = springs;
		const lift = still ? 0 : Math.min(1, Math.hypot(x.velocity, y.velocity) / LIFT_SPEED);
		const lean: [number, number] = still ? [0, 0] : [-clamp(y.velocity / LEAN_SPEED) * LEAN, clamp(x.velocity / LEAN_SPEED) * LEAN];
		const size = springs.size.value * (1 + 0.04 * lift);
		const card: FlyerCard = {
			key: FLIGHT,
			src: this.#src,
			base: this.#base,
			// Its own camera, straight on: flat, it lands on exactly the page's pixels.
			eye: [x.value, y.value],
			perspective: size * 3,
			centre: [0, 0, 0],
			angle: 0,
			turn: lean,
			size,
			radius: Math.max(0, springs.radius.value),
			reflection: Math.max(0, springs.reflection.value),
			shadow: Math.max(0, springs.shadow.value) + 0.8 * lift,
			ring: clamp(springs.ring.value, 0, 1),
			shade: 1,
			opacity: clamp(springs.opacity.value, 0, 1)
		};
		this.#show([card]);
		if (this.#landing && PARTS.every((part) => settled(springs[part], WITHIN[part]))) return this.#finish();
		if (this.#landing) this.#raf = requestAnimationFrame(this.#frame);
	};

	/** Aims the springs at where the cover it lands on is now: the springs keep their speed, so it curves there rather than jumps. */
	#follow(springs: Record<(typeof PARTS)[number], Spring>) {
		const target = this.#target;
		if (!target?.el.isConnected) return;
		const r = target.el.getBoundingClientRect();
		if (!r.width) return;
		springs.x.target = r.left + r.width / 2;
		springs.y.target = r.top + r.height / 2;
		springs.size.target = r.width;
		springs.radius.target = (target.pose.radius * r.width) / target.pose.size;
		// As sharp as it will be there, once the page has that image.
		const img = best(target.el);
		if (img && img.naturalWidth > this.#width) {
			this.#width = img.naturalWidth;
			flyers.sharpen(FLIGHT, img.currentSrc);
		}
	}

	/** Lands at once: the page (or the flow) shows its cover again, and the flying one goes a frame later. */
	#finish() {
		if (!this.#springs) return;
		cancelAnimationFrame(this.#raf);
		clearTimeout(this.#stranded);
		this.#raf = 0;
		this.#last = 0;
		this.#springs = null;
		this.#target = null;
		this.key = null;
		stage().draw();
		// One more frame in the air, so the page's cover is up before it goes.
		const flight = this.#flights;
		this.#touchDown(() => flight === this.#flights && !this.#springs && !this.#switching);
	}

	/** Slot `key`'s cover in sight (the phone's mini player or the desktop's, whichever shows). */
	#find(key: string): Slot | undefined {
		for (const slot of this.#slots.get(key) ?? []) {
			const r = slot.el.getBoundingClientRect();
			if (r.width && r.bottom > 0 && r.top < innerHeight && r.right > 0 && r.left < innerWidth) return slot;
		}
	}
}

export const flight = new Flight();

/** The part of the screen the bars over the page (marked data-bar) leave open: its top and bottom. */
function open(): [number, number] {
	let [top, bottom] = [0, innerHeight];
	for (const bar of document.querySelectorAll('[data-bar]')) {
		const r = bar.getBoundingClientRect();
		if (!r.height) continue;
		if (r.top + r.height / 2 < innerHeight / 2) top = Math.max(top, r.bottom);
		else bottom = Math.min(bottom, r.top);
	}
	return [top, bottom];
}

/** A cover lying flat at `pose`, seen straight on. */
function flat(pose: Pose): Place {
	return {
		eye: [pose.x, pose.y],
		perspective: pose.size * 3,
		centre: [0, 0, 0],
		angle: 0,
		turn: [0, 0],
		size: pose.size,
		radius: pose.radius,
		reflection: pose.reflection,
		shadow: pose.shadow,
		ring: pose.ring,
		shade: 1
	};
}

/** Sets where mover `m` flies from and to, both seen through one camera: flat covers land on the same pixels under any. */
function aim(m: Mover) {
	const goal = m.goal!;
	m.from = goal.flow && !m.origin.flow ? seenFrom(m.origin, goal) : m.origin;
	m.to = goal.flow || !m.origin.flow ? goal : seenFrom(goal, m.origin);
}

/** Flat `place` as seen through `other`'s camera: the same pixels, so the two can be mixed. */
function seenFrom(place: Place, other: Place): Place {
	const [x, y] = [place.eye[0] + place.centre[0], place.eye[1] + place.centre[1]];
	return { ...place, eye: other.eye, perspective: other.perspective, centre: [x - other.eye[0], y - other.eye[1], 0], flow: other.flow };
}

/** `t` of the way from `a` to `b` (beyond 1 when a spring overshoots). */
function mix(a: Place, b: Place, t: number): Place {
	const n = (x: number, y: number) => x + (y - x) * t;
	return {
		eye: [n(a.eye[0], b.eye[0]), n(a.eye[1], b.eye[1])],
		perspective: n(a.perspective, b.perspective),
		centre: [n(a.centre[0], b.centre[0]), n(a.centre[1], b.centre[1]), n(a.centre[2], b.centre[2])],
		angle: n(a.angle, b.angle),
		turn: [n(a.turn[0], b.turn[0]), n(a.turn[1], b.turn[1])],
		size: n(a.size, b.size),
		radius: Math.max(0, n(a.radius, b.radius)),
		reflection: Math.max(0, n(a.reflection, b.reflection)),
		shadow: Math.max(0, n(a.shadow, b.shadow)),
		ring: clamp(n(a.ring, b.ring), 0, 1),
		shade: n(a.shade, b.shade),
		flow: b.flow
	};
}

/** Where the cover in `el` is and how it looks: its corners, shadow and ring as its classes have them. */
function poseOf(el: HTMLElement): Pose {
	const r = el.getBoundingClientRect();
	// Scaled (Now Playing's cover, paused): the corners scale with it.
	const scale = el.offsetWidth ? r.width / el.offsetWidth : 1;
	const reflection = el.closest<HTMLElement>('[data-reflection]')?.dataset.reflection;
	return {
		x: r.left + r.width / 2,
		y: r.top + r.height / 2,
		size: r.width,
		radius: (parseFloat(getComputedStyle(el).borderTopLeftRadius) || 0) * scale,
		shadow: el.classList.contains('shadow-cover') ? 1 : 0,
		ring: el.classList.contains('ring-1') ? 1 : 0,
		reflection: Number(reflection ?? 0),
		opacity: 1
	};
}

/** The sharpest image in `el` that has loaded. */
function best(el: HTMLElement): HTMLImageElement | undefined {
	let found: HTMLImageElement | undefined;
	for (const img of el.querySelectorAll('img')) {
		if (img.complete && img.naturalWidth > (found?.naturalWidth ?? 0)) found = img;
	}
	return found;
}
