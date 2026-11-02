// Glitter bursting out of a cover (the dice's pick in the cover flow): a few hundred flakes of foil
// thrown into the air, braked hard by it and drifting down on a gusty breeze, tumbling as they fall
// (which sends them gliding sideways) and flashing as they turn to the light. Some land on the
// buttons and the player below and lie there glinting, with the phone's tilt where motion is on,
// before they fade. Every burst is thrown its own way, on its own wind, in its own colours: often
// the cover's. Drawn on a 2D canvas over the page while any is in the air, then gone.

import { motion } from './motion.svelte.ts';
import { mix, type Rgb } from './palette.ts';

/** Gravity, px/s²: slow, glitter in a snow globe rather than sand. */
const G = 560;
/** How much a tilted flake slides sideways through the air, against its drag. */
const GLIDE = 0.55;
/** The light falls from the upper left, in front of the screen. */
const LIGHT: [number, number, number] = [-0.32, -0.72, 0.62];
/** Off-screen by this much, a flake is gone. */
const MARGIN = 60;

type Shape = 'hex' | 'square' | 'strip' | 'star';
type Throw = 'burst' | 'fountain' | 'swirl' | 'cannons' | 'pop';

interface Flake {
	x: number;
	y: number;
	vx: number;
	vy: number;
	/** Across, px. */
	size: number;
	shape: Shape;
	/** Its turn in the screen's plane, and how fast it turns (rad, rad/s). */
	spin: number;
	spinRate: number;
	/** Its flip about its own long axis: face-on to the light, or edge-on. */
	flip: number;
	flipRate: number;
	/** A lean out of the flip's plane, its own: not every flake flashes at the same angle. */
	lean: number;
	/** Air drag face-on to the air: a = k·|v|·v. */
	drag: number;
	/** How sharp its highlight is (a specular exponent). */
	shine: number;
	color: Rgb;
	/** Rainbow foil: its colour turns with the light (a hue, degrees), or null. */
	hue: number | null;
	flare: HTMLCanvasElement;
	/** Seconds into the glitter's life: when it comes out, and when it is gone. */
	born: number;
	dies: number;
	/** Lying on a ledge: since when, or 0. */
	rest: number;
	bounces: number;
}

/** A still twinkle on the cover's face. */
interface Glint {
	x: number;
	y: number;
	size: number;
	born: number;
	life: number;
	flare: HTMLCanvasElement;
}

/** The top edge of something flakes land on. */
interface Ledge {
	top: number;
	left: number;
	right: number;
}

interface Wind {
	/** Steady breeze, px/s. */
	x: number;
	/** Gusts: how strong, how fine, and where they start. */
	gust: number;
	scale: number;
	phase: [number, number, number, number];
	/** A whirl around the cover (rad/s, fading), for a swirl. */
	whirl: number;
	cx: number;
	cy: number;
	radius: number;
}

const FOILS: Record<string, Rgb[]> = {
	gold: [
		[246, 211, 101],
		[226, 176, 64],
		[255, 236, 170],
		[196, 146, 46]
	],
	silver: [
		[232, 236, 242],
		[198, 206, 218],
		[250, 250, 255],
		[172, 182, 198]
	],
	rose: [
		[246, 180, 160],
		[234, 142, 142],
		[255, 216, 198],
		[216, 122, 152]
	],
	candy: [
		[255, 105, 180],
		[110, 196, 255],
		[255, 220, 90],
		[164, 120, 255],
		[96, 228, 178]
	]
};
const GOLD = FOILS.gold[0];
const WHITE: Rgb = [255, 255, 255];

const random = (from: number, to: number) => from + Math.random() * (to - from);
const pick = <T>(list: readonly T[]): T => list[Math.floor(Math.random() * list.length)];

/** Shapes a flake can have, half a unit across: drawn scaled to its size. */
const paths: Record<Shape, Path2D> | null =
	typeof Path2D === 'undefined'
		? null
		: {
				hex: polygon(6, () => 0.5),
				square: rect(1, 1),
				strip: rect(1, 0.38),
				star: polygon(8, (i) => (i % 2 ? 0.2 : 0.55))
			};

function polygon(n: number, radius: (i: number) => number): Path2D {
	const path = new Path2D();
	for (let i = 0; i < n; i++) {
		const a = (i / n) * Math.PI * 2;
		path[i ? 'lineTo' : 'moveTo'](Math.cos(a) * radius(i), Math.sin(a) * radius(i));
	}
	path.closePath();
	return path;
}

function rect(w: number, h: number): Path2D {
	const path = new Path2D();
	path.rect(-w / 2, -h / 2, w, h);
	return path;
}

/** A star of light: a soft halo in the flake's colour, a white core and four rays (a camera's, not turning). */
function flareOf([r, g, b]: Rgb): HTMLCanvasElement {
	const canvas = document.createElement('canvas');
	canvas.width = canvas.height = 64;
	const ctx = canvas.getContext('2d')!;
	const halo = ctx.createRadialGradient(32, 32, 0, 32, 32, 32);
	halo.addColorStop(0, 'rgba(255,255,255,0.95)');
	halo.addColorStop(0.1, `rgba(${r},${g},${b},0.6)`);
	halo.addColorStop(0.4, `rgba(${r},${g},${b},0.14)`);
	halo.addColorStop(1, `rgba(${r},${g},${b},0)`);
	ctx.fillStyle = halo;
	ctx.fillRect(0, 0, 64, 64);
	ctx.translate(32, 32);
	for (const [length, width, angle] of [
		[31, 1.4, 0],
		[31, 1.4, Math.PI / 2],
		[15, 1, Math.PI / 4],
		[15, 1, -Math.PI / 4]
	]) {
		ctx.save();
		ctx.rotate(angle);
		const ray = ctx.createLinearGradient(-length, 0, length, 0);
		ray.addColorStop(0, 'rgba(255,255,255,0)');
		ray.addColorStop(0.5, 'rgba(255,255,255,0.95)');
		ray.addColorStop(1, 'rgba(255,255,255,0)');
		ctx.fillStyle = ray;
		ctx.beginPath();
		ctx.moveTo(-length, 0);
		ctx.lineTo(0, -width);
		ctx.lineTo(length, 0);
		ctx.lineTo(0, width);
		ctx.fill();
		ctx.restore();
	}
	return canvas;
}

/** An HSL hue at full colour, for rainbow foil. */
function hueRgb(hue: number): Rgb {
	const h = (((hue % 360) + 360) % 360) / 60;
	const x = 1 - Math.abs((h % 2) - 1);
	const [r, g, b] = h < 1 ? [1, x, 0] : h < 2 ? [x, 1, 0] : h < 3 ? [0, 1, x] : h < 4 ? [0, x, 1] : h < 5 ? [x, 0, 1] : [1, 0, x];
	// Pastel, as foil is: never far from white.
	return [150 + r * 105, 150 + g * 105, 150 + b * 105];
}

/** The burst's colours: the cover's, made metallic with a touch of gold, or a foil of its own. */
function colorsFor(cover: Rgb[] | null, dark: boolean): { colors: Rgb[]; rainbow: number } {
	const prism = Math.random() < 0.12;
	// A cover in greys and beiges would make dull glitter: foil instead.
	const colourful = cover?.some((c) => Math.max(...c) - Math.min(...c) > 60);
	if (cover && colourful && Math.random() < 0.55) {
		const bright = cover.map((c) => {
			const light = 0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2];
			return mix(c, WHITE, light < 80 ? 0.45 : 0.2);
		});
		return { colors: [...bright, ...bright, GOLD], rainbow: prism ? 1 : random(0, 0.2) };
	}
	// Silver on a light page is only grey.
	const foils = Object.entries(FOILS).filter(([name]) => dark || name !== 'silver');
	return { colors: pick(foils)[1], rainbow: prism ? 1 : random(0, 0.3) };
}

class Glitter {
	#canvas: HTMLCanvasElement | null = null;
	#ctx: CanvasRenderingContext2D | null = null;
	#scale = 1;
	#flakes: Flake[] = [];
	#glints: Glint[] = [];
	#ledges: Ledge[] = [];
	#wind: Wind | null = null;
	/** Seconds since the glitter started. */
	#time = 0;
	#last = 0;
	#frame = 0;
	#dark = false;
	#tilt: [number, number] = [0, 0];
	#untilt: (() => void) | null = null;

	/**
	 * Throws glitter out of the cover at `from` (on the screen), to land on the top edges of
	 * `ledges`. `cover`: its colours, if known. With Reduce Motion, only twinkles on the cover.
	 */
	burst(from: DOMRect, { cover = null, ledges = [], dark = false }: { cover?: Rgb[] | null; ledges?: Element[]; dark?: boolean } = {}) {
		if (!paths || !from.width) return;
		this.#start(dark);
		const t = this.#time;
		const { colors, rainbow } = colorsFor(cover, dark);
		const flares = new Map(colors.map((c) => [c, flareOf(c)]));
		const whiteFlare = flareOf(mix(pick(colors), WHITE, 0.5));
		const [cx, cy, h] = [from.left + from.width / 2, from.top + from.height / 2, from.width / 2];

		for (let i = 0, n = Math.round(random(10, 18)); i < n; i++)
			this.#glints.push({
				x: cx + random(-h, h) * 0.9,
				y: cy + random(-h, h) * 0.9,
				size: random(14, 34),
				born: t + random(0.02, 1.1),
				life: random(0.35, 0.75),
				flare: Math.random() < 0.5 ? whiteFlare : flares.get(pick(colors))!
			});
		if (matchMedia('(prefers-reduced-motion: reduce)').matches) return;

		this.#ledges = ledges.flatMap((el) => {
			const r = el.getBoundingClientRect();
			// Rounded: only the flat part of the top edge holds anything.
			const corner = Math.min(r.height / 2, parseFloat(getComputedStyle(el).borderTopLeftRadius) || 0) * 0.7;
			return r.width > 0 && r.height > 0 ? [{ top: r.top, left: r.left + corner, right: r.right - corner }] : [];
		});
		const style: Throw = pick(['burst', 'burst', 'fountain', 'swirl', 'cannons', 'pop']);
		this.#wind = {
			x: random(-70, 70),
			gust: random(20, 90),
			scale: random(0.006, 0.014),
			phase: [random(0, 6), random(0, 6), random(0, 6), random(0, 6)],
			whirl: style === 'swirl' ? pick([-1, 1]) * random(3, 5.5) : 0,
			cx,
			cy,
			radius: h * 2.2
		};
		// Which shapes this glitter is cut in: mostly one, some of another.
		const shapes: Shape[] = [pick(['hex', 'hex', 'square']), pick(['hex', 'square', 'strip']), pick(['strip', 'star'])];
		const count = Math.round(random(240, 400) * (style === 'pop' ? 0.85 : 1));
		const sizing = random(1.6, 2.6);

		for (let i = 0; i < count; i++) {
			const launch = this.#launch(style, cx, cy, h, i / count);
			const shape = Math.random() < 0.7 ? shapes[0] : Math.random() < 0.8 ? shapes[1] : shapes[2];
			const color = pick(colors);
			const holo = Math.random() < rainbow;
			const fall = random(150, 320);
			const big = shape === 'star' ? random(7, 11) : shape === 'strip' ? random(5, 9) : 2 + Math.random() ** sizing * 6;
			this.#flakes.push({
				...launch,
				born: t + launch.delay,
				dies: t + launch.delay + random(3.5, 6.5),
				size: big,
				shape,
				spin: random(0, Math.PI * 2),
				spinRate: random(-9, 9),
				flip: random(0, Math.PI * 2),
				flipRate: pick([-1, 1]) * random(3, 14),
				lean: random(-0.6, 0.6),
				drag: G / (fall * fall),
				shine: random(24, 90),
				color,
				hue: holo ? random(0, 360) : null,
				flare: holo ? whiteFlare : flares.get(color)!,
				rest: 0,
				bounces: 0
			});
		}
	}

	/** What the glitter lay on is gone (the page changed): it falls off. */
	drop() {
		this.#ledges = [];
		for (const f of this.#flakes) {
			if (!f.rest) continue;
			f.rest = 0;
			f.flipRate = pick([-1, 1]) * random(2, 6);
			f.dies = this.#time + random(2, 3.5);
		}
	}

	/** Where a flake leaves the cover from, how fast, and when (s after the burst). */
	#launch(style: Throw, cx: number, cy: number, h: number, nth: number) {
		// A point on the cover's edge, and the way out from its centre.
		const edge = () => {
			const side = Math.floor(Math.random() * 4);
			const along = random(-h, h);
			const [x, y] = [
				[along, -h],
				[h, along],
				[along, h],
				[-h, along]
			][side];
			const d = Math.hypot(x, y);
			return { x: cx + x, y: cy + y, ux: x / d, uy: y / d };
		};
		const turn = (ux: number, uy: number, by: number) => [ux * Math.cos(by) - uy * Math.sin(by), ux * Math.sin(by) + uy * Math.cos(by)];
		// Most flakes go gently, a few fly far.
		const speed = (from: number, to: number) => from + (to - from) * Math.random() ** 1.6;

		switch (style) {
			case 'fountain': {
				const along = random(-0.85, 0.85);
				const [ux, uy] = turn(0, -1, along * 0.55 + random(-0.25, 0.25));
				const v = speed(800, 1800);
				return { x: cx + along * h, y: cy - h, vx: ux * v, vy: uy * v, delay: nth * 0.45 + random(0, 0.05) };
			}
			case 'swirl': {
				const e = edge();
				const way = this.#wind!.whirl > 0 ? 1 : -1;
				const [ux, uy] = [e.ux * 0.55 - e.uy * way * 0.85, e.uy * 0.55 + e.ux * way * 0.85];
				const v = speed(600, 1400);
				return { x: e.x, y: e.y, vx: ux * v, vy: uy * v - 150, delay: random(0, 0.3) };
			}
			case 'cannons': {
				const side = nth < 0.5 ? -1 : 1;
				const angle = (random(58, 78) * Math.PI) / 180;
				const v = speed(1100, 2100);
				return {
					x: cx + side * h * 0.95,
					y: cy + h * 0.6,
					vx: side * Math.cos(angle) * v,
					vy: -Math.sin(angle) * v,
					delay: random(0, 0.35)
				};
			}
			case 'pop': {
				// Out of its face, in two waves.
				const [x, y] = [random(-h, h) * 0.8, random(-h, h) * 0.8];
				const d = Math.hypot(x, y) || 1;
				const v = 450 + (d / h) * speed(500, 1300);
				return { x: cx + x, y: cy + y, vx: (x / d) * v, vy: (y / d) * v - 220, delay: nth < 0.7 ? random(0, 0.06) : random(0.22, 0.3) };
			}
			default: {
				const e = edge();
				const [ux, uy] = turn(e.ux, e.uy, random(-0.5, 0.5));
				const v = speed(450, 1500);
				return { x: e.x, y: e.y, vx: ux * v, vy: uy * v - 250, delay: random(0, 0.12) };
			}
		}
	}

	#start(dark: boolean) {
		this.#dark = dark;
		const scale = Math.min(3, devicePixelRatio || 1);
		if (!this.#canvas) {
			const canvas = document.createElement('canvas');
			canvas.className = 'pointer-events-none fixed inset-0 z-[38] size-full';
			canvas.setAttribute('aria-hidden', 'true');
			document.body.append(canvas);
			this.#canvas = canvas;
			this.#ctx = canvas.getContext('2d');
			this.#time = 0;
			this.#untilt = motion.subscribe((x, y) => (this.#tilt = [x, y]));
		}
		this.#scale = scale;
		this.#canvas.width = Math.round(innerWidth * scale);
		this.#canvas.height = Math.round(innerHeight * scale);
		if (!this.#frame) {
			this.#last = performance.now();
			this.#frame = requestAnimationFrame(this.#tick);
		}
	}

	#stop() {
		cancelAnimationFrame(this.#frame);
		this.#frame = 0;
		this.#canvas?.remove();
		[this.#canvas, this.#ctx] = [null, null];
		this.#untilt?.();
		this.#untilt = null;
		this.#tilt = [0, 0];
		[this.#flakes, this.#glints, this.#ledges, this.#wind] = [[], [], [], null];
	}

	#tick = (now: number) => {
		// After a stall (a hidden tab), on from where it was rather than all at once.
		const dt = Math.min(1 / 30, Math.max(0, (now - this.#last) / 1000));
		this.#last = now;
		this.#time += dt;
		this.#step(dt);
		this.#draw();
		if (this.#flakes.length || this.#glints.length) this.#frame = requestAnimationFrame(this.#tick);
		else this.#stop();
	};

	/** The air's own movement at `(x, y)`: the breeze, its gusts and a whirl, px/s. */
	#air(x: number, y: number): [number, number] {
		const w = this.#wind;
		if (!w) return [0, 0];
		const t = this.#time;
		const [a, b, c, d] = w.phase;
		let ux = w.x + w.gust * Math.sin(x * w.scale + t * 0.9 + a) * Math.cos(y * w.scale * 0.8 - t * 0.6 + b);
		let uy = w.gust * 0.6 * Math.cos(x * w.scale * 0.7 - t * 0.7 + c) * Math.sin(y * w.scale + t * 0.8 + d);
		if (w.whirl) {
			const [dx, dy] = [x - w.cx, y - w.cy];
			const k = w.whirl * Math.exp(-(dx * dx + dy * dy) / (w.radius * w.radius) - t / 1.4);
			ux -= dy * k;
			uy += dx * k;
		}
		return [ux, uy];
	}

	#step(dt: number) {
		const t = this.#time;
		const [width, height] = [innerWidth, innerHeight];
		this.#glints = this.#glints.filter((g) => t < g.born + g.life);
		this.#flakes = this.#flakes.filter((f) => {
			if (t >= f.dies || f.y > height + MARGIN || f.x < -MARGIN || f.x > width + MARGIN) return false;
			if (f.rest || t < f.born) return true;
			// Drag works on the flake's speed through the air, so the wind carries it. Face-on to the
			// air it brakes harder; tilted, it slides off sideways (and its tilt keeps changing, so
			// it rocks from side to side as it falls).
			const [ax, ay] = this.#air(f.x, f.y);
			const [rx, ry] = [f.vx - ax, f.vy - ay];
			const speed = Math.hypot(rx, ry);
			const k = f.drag * (0.45 + 0.9 * Math.abs(Math.cos(f.flip))) * speed;
			const glide = GLIDE * f.drag * speed * Math.sin(2 * f.flip);
			// Drag taken implicitly: stable even for a flake thrown fast, at a long frame.
			f.vx = (f.vx + dt * (-glide * ry + k * ax)) / (1 + k * dt);
			f.vy = (f.vy + dt * (G + glide * rx + k * ay)) / (1 + k * dt);
			const before = f.y;
			f.x += f.vx * dt;
			f.y += f.vy * dt;
			// It tumbles faster the faster it goes.
			const pace = 0.35 + speed / 500;
			f.flip += f.flipRate * pace * dt;
			f.spin += f.spinRate * pace * dt;
			if (f.vy > 0) for (const ledge of this.#ledges) if (before <= ledge.top && f.y >= ledge.top && f.x > ledge.left && f.x < ledge.right) this.#land(f, ledge);
			return true;
		});
	}

	/** A flake hits a ledge: it bounces a little, or settles where it fell, lying nearly flat. */
	#land(f: Flake, ledge: Ledge) {
		if (f.vy > 160 && f.bounces < 2 && Math.random() < 0.7) {
			f.vy *= -random(0.12, 0.35);
			f.vx *= 0.5;
			f.y = ledge.top - 0.5;
			f.flipRate *= 0.6;
			f.bounces++;
			return;
		}
		f.y = ledge.top - 0.6;
		f.vx = f.vy = 0;
		f.rest = this.#time;
		f.dies = Math.min(f.dies + 1, this.#time + random(1.2, 4));
		// Facing up, a little towards the screen: seen nearly edge-on, a sliver catching the light.
		f.flip = -(Math.PI / 2 - random(0.2, 1.1));
		f.spin = random(-0.3, 0.3);
	}

	#draw() {
		const ctx = this.#ctx;
		const canvas = this.#canvas;
		if (!ctx || !canvas) return;
		const t = this.#time;
		const s = this.#scale;
		ctx.setTransform(1, 0, 0, 1, 0, 0);
		ctx.clearRect(0, 0, canvas.width, canvas.height);

		// The light moves a little by itself and with the phone, so even flakes lying still glint.
		const [mx, my] = this.#tilt;
		const L = unit([LIGHT[0] + mx * 0.6 + Math.sin(t * 0.8) * 0.15, LIGHT[1] + my * 0.6 + Math.cos(t * 0.6) * 0.12, LIGHT[2]]);
		const H = unit([L[0], L[1], L[2] + 1]);
		// Foil turned away from the light still mirrors what is around it: on a light page, light.
		const ambient = this.#dark ? 0.3 : 0.55;
		const flares: [Flake, number, number][] = [];

		for (const f of this.#flakes) {
			if (t < f.born) continue;
			const fade = Math.min(1, (f.dies - t) / 0.6);
			// Its normal: flipped about its own axis, leaning out of that plane, turned with it.
			const [cf, sf] = [Math.cos(f.flip), Math.sin(f.flip)];
			const [lx, ly, lz] = unit([Math.sin(f.lean) * cf, sf, Math.cos(f.lean) * cf]);
			const [cs, ss] = [Math.cos(f.spin), Math.sin(f.spin)];
			let n = [lx * cs - ly * ss, lx * ss + ly * cs, lz];
			if (n[2] < 0) n = [-n[0], -n[1], -n[2]];
			const diffuse = ambient + (1 - ambient) * Math.max(0, dot(n, L));
			const spec = Math.max(0, dot(n, H)) ** f.shine;
			const base = f.hue === null ? f.color : hueRgb(f.hue + 220 * (n[0] - n[1]));
			const r = Math.min(255, base[0] * diffuse + (255 - base[0] * diffuse) * spec);
			const g = Math.min(255, base[1] * diffuse + (255 - base[1] * diffuse) * spec);
			const b = Math.min(255, base[2] * diffuse + (255 - base[2] * diffuse) * spec);
			// Seen edge-on, it narrows to a sliver.
			const size = f.size * s;
			const face = Math.max(0.12, Math.abs(cf));
			ctx.setTransform(cs * size, ss * size, -ss * size * face, cs * size * face, f.x * s, f.y * s);
			ctx.globalAlpha = fade;
			ctx.fillStyle = `rgb(${r | 0},${g | 0},${b | 0})`;
			ctx.fill(paths![f.shape]);
			if (spec > 0.35 && f.size >= 2.2) flares.push([f, spec, fade]);
		}

		// Flashes, on top, adding light where the page is dark.
		ctx.setTransform(s, 0, 0, s, 0, 0);
		ctx.globalCompositeOperation = this.#dark ? 'lighter' : 'source-over';
		for (const [f, spec, fade] of flares) {
			const r = f.size * (1.4 + 5 * spec);
			ctx.globalAlpha = fade * ((spec - 0.35) / 0.65) ** 1.3;
			ctx.drawImage(f.flare, f.x - r, f.y - r, r * 2, r * 2);
		}
		for (const g of this.#glints) {
			if (t < g.born) continue;
			const k = Math.sin((Math.PI * (t - g.born)) / g.life) ** 2;
			const r = (g.size / 2) * (0.6 + 0.4 * k);
			ctx.globalAlpha = k;
			ctx.drawImage(g.flare, g.x - r, g.y - r, r * 2, r * 2);
		}
		ctx.globalCompositeOperation = 'source-over';
		ctx.globalAlpha = 1;
	}
}

function dot(a: number[], b: number[]): number {
	return a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
}

function unit([x, y, z]: number[]): [number, number, number] {
	const d = Math.hypot(x, y, z) || 1;
	return [x / d, y / d, z / d];
}

export const glitter = new Glitter();
