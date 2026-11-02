// Covers in the air between pages and views (lib/flight.svelte.ts), as images over the page. Each
// moves only by its transform and opacity, which WebKit animates on the GPU. Each is a card as the
// cover flow's renderer draws one (lib/stage-gl.ts), and its matrix projects as that renderer does:
// a cover leaves the flow, and lands in it, on the pixels the flow draws it on.

import { markDecoded } from './artwork.ts';
import { GAP, REFLECTION } from './coverflow.ts';
import type { Card } from './stage-gl.ts';

/** A cover in the air: where it is and how it looks, as a card, and its image. */
export type FlyerCard = Omit<Card, 'art' | 'light'> & {
	/** The same cover from one frame to the next. */
	key: string;
	src: string;
	/** Its size on the page, unscaled: its largest on the way, so it is never blown up. */
	base: number;
};

/** A 4×4 matrix, column by column, as matrix3d() takes it. */
type Matrix = number[];

const identity = (): Matrix => [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];

function multiply(a: Matrix, b: Matrix): Matrix {
	const out = new Array<number>(16);
	for (let c = 0; c < 4; c++) {
		for (let r = 0; r < 4; r++) {
			let sum = 0;
			for (let k = 0; k < 4; k++) sum += a[k * 4 + r] * b[c * 4 + k];
			out[c * 4 + r] = sum;
		}
	}
	return out;
}

const translate = (x: number, y: number, z = 0): Matrix => Object.assign(identity(), { 12: x, 13: y, 14: z });
const scale = (k: number): Matrix => Object.assign(identity(), { 0: k, 5: k });
const perspective = (d: number): Matrix => Object.assign(identity(), { 11: -1 / d });
const rotateX = (a: number): Matrix => Object.assign(identity(), { 5: Math.cos(a), 6: Math.sin(a), 9: -Math.sin(a), 10: Math.cos(a) });
const rotateY = (a: number): Matrix => Object.assign(identity(), { 0: Math.cos(a), 2: -Math.sin(a), 8: Math.sin(a), 10: Math.cos(a) });

/**
 * The transform that puts a `base`-sized box (its origin at its top left) where `card` is on the
 * screen: from the eye, turned, set back and seen in perspective, as CSS would have
 * `translate(eye) perspective(…) rotateX(…) rotateY(…) translate3d(centre) rotateY(angle) scale(…)`.
 */
export function cardMatrix(card: Pick<Card, 'eye' | 'perspective' | 'centre' | 'angle' | 'turn' | 'size'>, base: number): Matrix {
	return [
		translate(card.eye[0], card.eye[1]),
		perspective(card.perspective),
		rotateX(card.turn[0]),
		rotateY(card.turn[1]),
		translate(...card.centre),
		rotateY(card.angle),
		scale(card.size / base),
		translate(-base / 2, -base / 2)
	].reduce(multiply);
}

/** Where `m` takes the point `(x, y)` of the box. */
export function apply(m: Matrix, x: number, y: number): [number, number] {
	const w = m[3] * x + m[7] * y + m[15];
	return [(m[0] * x + m[4] * y + m[12]) / w, (m[1] * x + m[5] * y + m[13]) / w];
}

/** The parts of a cover in the air besides its image, each made only once a card needs it: every one is a layer of its own. */
type Part = 'shadow' | 'lifted' | 'ring' | 'shade' | 'reflection';

interface Flyer {
	root: HTMLDivElement;
	cover: HTMLDivElement;
	img: HTMLImageElement;
	/** A sharper image of it, shown over the first once decoded. */
	sharp: HTMLImageElement;
	parts: Partial<Record<Part, HTMLDivElement>>;
	/** The reflection's image. */
	mirror: HTMLImageElement | null;
	base: number;
	/** What was last written, so a frame writes only what changed. */
	written: Record<string, string>;
}

const CORNERED = 'position:absolute;inset:0;border-radius:inherit';

const element = <K extends keyof HTMLElementTagNameMap>(tag: K, css: string) => {
	const node = document.createElement(tag);
	node.style.cssText = css;
	return node;
};

const image = (css: string) => {
	const img = element('img', css);
	img.alt = '';
	img.decoding = 'sync';
	img.draggable = false;
	return img;
};

function make(card: FlyerCard): Flyer {
	const root = element('div', 'position:absolute;left:0;top:0;transform-origin:0 0;will-change:transform,opacity');
	const cover = element('div', `${CORNERED};overflow:hidden;background:var(--surface-2)`);
	const img = image('position:absolute;inset:0;width:100%;height:100%;object-fit:cover');
	const sharp = image('position:absolute;inset:0;width:100%;height:100%;object-fit:cover;opacity:0');
	cover.append(img, sharp);
	root.append(cover);
	const flyer: Flyer = { root, cover, img, sharp, parts: {}, mirror: null, base: 0, written: {} };
	resize(flyer, card.base);
	img.src = card.src;
	markDecoded(card.src);
	return flyer;
}

/** Makes part `name` of `f`, in its place among the others: the shadows under the cover, the ring and reflection over it, the shade on it. */
function part(f: Flyer, name: Part): HTMLDivElement {
	const made = f.parts[name];
	if (made) return made;
	let node: HTMLDivElement;
	if (name === 'shadow') node = element('div', `${CORNERED};box-shadow:var(--shadow-cover);will-change:opacity`);
	// More shadow, lifted.
	else if (name === 'lifted') node = element('div', `${CORNERED};box-shadow:var(--shadow-float);opacity:0;will-change:opacity`);
	else if (name === 'ring') node = element('div', `${CORNERED};box-shadow:0 0 0 1px var(--line);will-change:opacity`);
	else if (name === 'shade') node = element('div', 'position:absolute;inset:0;background:#000;opacity:0;will-change:opacity');
	else {
		// The cover's lower part upside down below it, fading out, as the flow draws it.
		node = element(
			'div',
			'position:absolute;left:0;top:100%;width:100%;overflow:hidden;border-radius:inherit;opacity:0;will-change:transform,opacity;' +
				'-webkit-mask-image:linear-gradient(rgb(0 0 0 / 0.3),transparent 85%);mask-image:linear-gradient(rgb(0 0 0 / 0.3),transparent 85%)'
		);
		f.mirror = image('position:absolute;left:0;top:0;width:100%;object-fit:cover;transform:scaleY(-1)');
		f.mirror.src = f.img.src;
		node.append(f.mirror);
	}
	f.parts[name] = node;
	if (name === 'shade') f.cover.append(node);
	else if (name === 'shadow') f.root.prepend(node);
	else if (name === 'lifted') f.root.insertBefore(node, f.cover);
	else if (name === 'ring') f.cover.after(node);
	else f.root.append(node);
	resize(f, f.base);
	return node;
}

function resize(f: Flyer, base: number) {
	f.base = base;
	f.root.style.width = f.root.style.height = `${base}px`;
	if (f.parts.reflection) f.parts.reflection.style.height = `${Math.round(base * REFLECTION)}px`;
	if (f.mirror) f.mirror.style.height = `${base}px`;
}

/**
 * Sets style `name` on `f`'s part `part` to `value`, unless it is that already. A part not made yet
 * is made only when it shows: `none` is how it looks when it doesn't.
 */
function write(f: Flyer, which: 'root' | Part, name: string, value: string, none?: string) {
	const slot = `${which} ${name}`;
	if (f.written[slot] === value) return;
	if (which !== 'root' && !f.parts[which] && value === none) return;
	f.written[slot] = value;
	(which === 'root' ? f.root : part(f, which)).style.setProperty(name, value);
}

const fixed = (n: number, digits = 3) => n.toFixed(digits);

class Flyers {
	#layer: HTMLDivElement | null = null;
	#flyers = new Map<string, Flyer>();

	/** Shows `cards` this frame, in order (the last on top), and drops the covers no longer among them. */
	set(cards: FlyerCard[]) {
		if (!cards.length) return this.clear();
		const layer = (this.#layer ??= this.#makeLayer());
		if (!layer.isConnected) document.body.append(layer);
		const keep = new Set(cards.map((card) => card.key));
		for (const [key, f] of this.#flyers) {
			if (keep.has(key)) continue;
			f.root.remove();
			this.#flyers.delete(key);
		}
		let previous: Element | null = null;
		for (const card of cards) {
			let f = this.#flyers.get(card.key);
			if (!f) this.#flyers.set(card.key, (f = make(card)));
			if (card.base > f.base) resize(f, card.base);
			// Another cover (taken off again as the last one landed).
			if (f.img.getAttribute('src') !== card.src) {
				f.img.src = card.src;
				if (f.mirror) f.mirror.src = card.src;
				f.sharp.removeAttribute('src');
				f.sharp.style.opacity = '0';
				markDecoded(card.src);
			}
			const next: Element | null = previous ? previous.nextElementSibling : layer.firstElementChild;
			if (next !== f.root) layer.insertBefore(f.root, next);
			previous = f.root;
			this.#draw(f, card);
		}
	}

	/** Shows only the part of the screen from `top` to `bottom`, or all of it. */
	clip(range: [top: number, bottom: number] | null) {
		const layer = (this.#layer ??= this.#makeLayer());
		layer.style.clipPath = range ? `inset(${range[0]}px 0 ${Math.max(0, innerHeight - range[1])}px 0)` : '';
	}

	/** Shows a sharper image of cover `key` once it is ready, over the one it flies with. */
	sharpen(key: string, src: string) {
		const f = this.#flyers.get(key);
		if (!f || f.sharp.getAttribute('src') === src || f.img.getAttribute('src') === src) return;
		f.sharp.style.opacity = '0';
		f.sharp.src = src;
		f.sharp.decode().then(
			() => f.sharp.getAttribute('src') === src && (f.sharp.style.opacity = '1'),
			() => {}
		);
	}

	clear() {
		for (const f of this.#flyers.values()) f.root.remove();
		this.#flyers.clear();
		this.#layer?.remove();
		if (this.#layer) this.#layer.style.clipPath = '';
	}

	#makeLayer() {
		const layer = document.createElement('div');
		layer.className = 'pointer-events-none fixed inset-0 z-[35]';
		// Above the page's cross-fade, and live through it (app.css).
		layer.style.viewTransitionName = 'stage';
		layer.setAttribute('aria-hidden', 'true');
		return layer;
	}

	#draw(f: Flyer, card: FlyerCard) {
		const k = f.base / card.size;
		write(f, 'root', 'transform', `matrix3d(${cardMatrix(card, f.base).map((n) => fixed(n, 6)).join(',')})`);
		write(f, 'root', 'opacity', fixed(card.opacity));
		write(f, 'root', 'border-radius', `${fixed(card.radius * k, 1)}px`);
		const none = fixed(0);
		write(f, 'shadow', 'opacity', fixed(Math.min(1, card.shadow)), none);
		write(f, 'lifted', 'opacity', fixed(Math.max(0, Math.min(1, card.shadow - 1))), none);
		write(f, 'ring', 'opacity', fixed(card.ring), none);
		write(f, 'shade', 'opacity', fixed(1 - card.shade), none);
		write(f, 'reflection', 'opacity', fixed(Math.min(1, card.reflection / (REFLECTION * card.size))), none);
		if (f.parts.reflection) write(f, 'reflection', 'transform', `translateY(${fixed(GAP * k, 2)}px)`);
	}
}

export const flyers = new Flyers();
