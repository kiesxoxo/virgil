// Covers in the air between pages and views (lib/flight.svelte.ts), as images over the page. Each
// moves only by its transform and opacity, which WebKit animates on the GPU. Each is a card as the
// cover flow's renderer draws one (lib/stage-gl.ts), and its matrix projects as that renderer does:
// a cover leaves the flow, and lands in it, on the pixels the flow draws it on.

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

interface Flyer {
	root: HTMLDivElement;
	img: HTMLImageElement;
	/** A sharper image of it, shown over the first once decoded. */
	sharp: HTMLImageElement;
	mirror: HTMLImageElement;
	shadow: HTMLDivElement;
	/** More shadow, lifted. */
	lifted: HTMLDivElement;
	ring: HTMLDivElement;
	shade: HTMLDivElement;
	reflection: HTMLDivElement;
	base: number;
	/** What was last written, so a frame writes only what changed. */
	written: Record<string, string>;
}

const CORNERED = 'position:absolute;inset:0;border-radius:inherit';

function make(card: FlyerCard): Flyer {
	const el = <K extends keyof HTMLElementTagNameMap>(tag: K, css: string) => {
		const node = document.createElement(tag);
		node.style.cssText = css;
		return node;
	};
	const root = el('div', 'position:absolute;left:0;top:0;transform-origin:0 0;will-change:transform,opacity');
	const shadow = el('div', `${CORNERED};box-shadow:var(--shadow-cover);will-change:opacity`);
	const lifted = el('div', `${CORNERED};box-shadow:var(--shadow-float);opacity:0;will-change:opacity`);
	const cover = el('div', `${CORNERED};overflow:hidden;background:var(--surface-2)`);
	const img = el('img', 'position:absolute;inset:0;width:100%;height:100%;object-fit:cover');
	const sharp = el('img', 'position:absolute;inset:0;width:100%;height:100%;object-fit:cover;opacity:0');
	const shade = el('div', 'position:absolute;inset:0;background:#000;opacity:0;will-change:opacity');
	const ring = el('div', `${CORNERED};box-shadow:0 0 0 1px var(--line);will-change:opacity`);
	// The cover's lower part upside down below it, fading out, as the flow draws it.
	const reflection = el(
		'div',
		'position:absolute;left:0;top:100%;width:100%;overflow:hidden;border-radius:inherit;opacity:0;will-change:transform,opacity;' +
			'-webkit-mask-image:linear-gradient(rgb(0 0 0 / 0.3),transparent 85%);mask-image:linear-gradient(rgb(0 0 0 / 0.3),transparent 85%)'
	);
	const mirror = el('img', 'position:absolute;left:0;top:0;width:100%;object-fit:cover;transform:scaleY(-1)');
	for (const image of [img, sharp, mirror]) {
		image.alt = '';
		image.decoding = 'sync';
		image.draggable = false;
	}
	cover.append(img, sharp, shade);
	reflection.append(mirror);
	root.append(shadow, lifted, cover, ring, reflection);
	const flyer: Flyer = { root, img, sharp, mirror, shadow, lifted, ring, shade, reflection, base: 0, written: {} };
	resize(flyer, card.base);
	img.src = mirror.src = card.src;
	return flyer;
}

function resize(f: Flyer, base: number) {
	f.base = base;
	f.root.style.width = f.root.style.height = `${base}px`;
	f.reflection.style.height = `${Math.round(base * REFLECTION)}px`;
	f.mirror.style.height = `${base}px`;
}

/** Sets style `name` on `f`'s part `part` to `value`, unless it is that already. */
function write(f: Flyer, part: 'root' | 'shadow' | 'lifted' | 'ring' | 'shade' | 'reflection', name: string, value: string) {
	const slot = `${part} ${name}`;
	if (f.written[slot] === value) return;
	f.written[slot] = value;
	f[part].style.setProperty(name, value);
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
				f.img.src = f.mirror.src = card.src;
				f.sharp.removeAttribute('src');
				f.sharp.style.opacity = '0';
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
		write(f, 'shadow', 'opacity', fixed(Math.min(1, card.shadow)));
		write(f, 'lifted', 'opacity', fixed(Math.max(0, Math.min(1, card.shadow - 1))));
		write(f, 'ring', 'opacity', fixed(card.ring));
		write(f, 'shade', 'opacity', fixed(1 - card.shade));
		write(f, 'reflection', 'opacity', fixed(Math.min(1, card.reflection / (REFLECTION * card.size))));
		write(f, 'reflection', 'transform', `translateY(${fixed(GAP * k, 2)}px)`);
	}
}

export const flyers = new Flyers();
