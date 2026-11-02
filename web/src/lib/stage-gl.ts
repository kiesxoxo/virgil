// Draws the cover flow with WebGL2. Every cover is a card: a quad seen through a camera, with its
// shadow and reflection. The canvas sits in the flow and scrolls with the page; where the flow's
// covers go comes from coverflow.ts. Covers flying to and from it (lib/flight.svelte.ts) are images
// over the page (lib/flyers.ts), turned and set back as these cards are, so they land on the same
// pixels. The renderer, its canvas and its textures outlive the flow on the page: coming back to it
// (and a page transition's picture of it) finds the covers already loaded.

import { markDecoded } from './artwork.ts';
import { GAP, backToFront, middle, place, turnAngles, visible, type Layout, type Turn } from './coverflow.ts';
import { TILE, artUrl, type ArtSize } from './format.ts';
import { hexRgb } from './palette.ts';

export interface Scene {
	layout: Layout;
	/** Scroll position, in covers. */
	position: number;
	count: number;
	/** Cover `index`'s artwork key. */
	art(index: number): string | null;
	/**
	 * How much of cover `index` the flow draws: all of it; only its reflection (the page draws
	 * the cover itself, for a page transition to move); or nothing (it is flying).
	 */
	drawn(index: number): 'all' | 'reflection' | 'none';
	/** At rest on a cover: the middle one gets the sharpest image. */
	settled: boolean;
	/** The phone's tilt: the flow turns with it, and the light on the covers moves. */
	turn: Turn;
	/** How far the middle cover comes out towards the viewer, px (the dice's pick, landing). */
	lift?: number;
	/**
	 * Where the flow is spinning to (in covers): its covers are fetched there, rather than the
	 * ones flashing by, which would hold up the ones it stops at.
	 */
	heading?: number;
}

/**
 * A cover drawn on its own: where its camera is, where the cover is from the camera, and how it
 * looks. A flat cover at depth 0 lands on the same pixels whatever the camera, so a cover can
 * fly from anywhere on the page into the flow's 3D and back.
 */
export interface Card {
	/** Its artwork key (or a full image URL, for radio artwork). */
	art: string;
	/** Where the camera looks from, in CSS pixels on the canvas, and how far back it is. */
	eye: [number, number];
	perspective: number;
	/** The cover's centre from the eye (CSS pixels, y down, z towards the viewer). */
	centre: [number, number, number];
	/** Its turn about its own vertical axis, in radians. */
	angle: number;
	/** Everything turned about the eye, as CSS rotateX and rotateY (radians). */
	turn: [number, number];
	size: number;
	radius: number;
	/** The reflection's height (0 for none). */
	reflection: number;
	/** 0 none … 1 the covers' usual shadow; more when lifted. */
	shadow: number;
	/** The thin ring around it, 0–1. */
	ring: number;
	/** Darker when turned away. */
	shade: number;
	opacity: number;
	/** The light on it as the phone moves: where it falls (0–1 each way) and how bright. */
	light: [number, number, number];
}

/** 0–1 each. */
type Rgba = [number, number, number, number];

export interface Theme {
	ring: Rgba;
	surface: Rgba;
	faint: Rgba;
	dark: boolean;
}

/** Simultaneous artwork downloads. */
const FETCHES = 4;
/** Textures uploaded per frame: each is a decoded image copied to the GPU, plus its mipmaps, a millisecond or more. */
const UPLOADS = 2;
/** Covers kept on the GPU, the least recently drawn going first. */
const KEEP = 32;
/** Covers beyond those in sight to load ahead, on each side. */
const AHEAD = 2;
/** An image fading in over its placeholder. */
const FADE_MS = 200;
/**
 * How long textures stay after the flow leaves the page, for coming back to it.
 * The flow's covers in sight stay on: back from an album, they would be placeholders as it came in.
 */
const LINGER_MS = 30_000;
/** The flow's corner radius, as rounded-xl. */
const RADIUS = 12;
const SIZES: ArtSize[] = [300, 600, 1000];

const VERTEX = `#version 300 es
	in vec2 a_corner;
	uniform vec4 u_rect;    // the quad, from the cover's centre: x, y, width, height
	uniform vec3 u_centre;  // the cover's centre from the eye
	uniform float u_angle;  // its turn about its own vertical axis
	uniform vec2 u_turn;    // everything turned about the eye, as CSS rotateX and rotateY
	uniform vec2 u_eye;     // where the camera looks from, on the canvas
	uniform float u_perspective;
	uniform vec2 u_canvas;  // the canvas's size
	out vec2 v_local;

	void main() {
		vec2 local = u_rect.xy + a_corner * u_rect.zw;
		float c = cos(u_angle), s = sin(u_angle);
		vec3 p = u_centre + vec3(local.x * c, local.y, -local.x * s);
		// As project() in coverflow.ts: about the vertical axis, then the horizontal one.
		float cy = cos(u_turn.y), sy = sin(u_turn.y), cx = cos(u_turn.x), sx = sin(u_turn.x);
		p = vec3(p.x * cy + p.z * sy, p.y, p.z * cy - p.x * sy);
		p = vec3(p.x, p.y * cx - p.z * sx, p.y * sx + p.z * cx);
		// Perspective as CSS does it, divided by the GPU so textures stay true across the quad.
		float w = (u_perspective - p.z) / u_perspective;
		vec2 screen = u_eye * w + p.xy;
		gl_Position = vec4(screen.x / u_canvas.x * 2.0 - w, w - screen.y / u_canvas.y * 2.0, 0.0, w);
		v_local = local;
	}
`;

const FRAGMENT = `#version 300 es
	precision highp float;

	in vec2 v_local;
	out vec4 o_color;

	uniform int u_mode; // 0 shadow, 1 cover, 2 reflection
	uniform float u_cover;
	uniform float u_radius;
	uniform float u_reflection;
	uniform sampler2D u_art;
	uniform float u_fade;   // 0 placeholder … 1 artwork
	uniform vec2 u_crop;    // the part of the image the cover shows, from its centre (as object-fit: cover)
	uniform float u_shade;
	uniform float u_alpha;
	uniform vec4 u_ring;    // premultiplied
	uniform vec3 u_surface;
	uniform vec3 u_faint;
	uniform vec2 u_shadow;  // opacity of the wide and the close shadow
	uniform vec3 u_light;   // where the light falls on a cover (0–1 each way) and how bright it is

	const float GAP = ${GAP.toFixed(1)};

	float box(vec2 p, vec2 b, float r) {
		vec2 q = abs(p) - b + r;
		return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r;
	}

	// How much of a pixel is inside the shape where d < 0: smooth edges at any angle.
	float fill(float d) {
		return clamp(0.5 - d / max(fwidth(d), 1e-3), 0.0, 1.0);
	}

	vec2 erf(vec2 x) {
		vec2 s = sign(x), a = abs(x);
		x = 1.0 + (0.278393 + (0.230389 + 0.078108 * (a * a)) * a) * a;
		x *= x;
		return s - s / (x * x);
	}

	// A box from -b to b blurred like a CSS shadow: a Gaussian of sigma = blur / 2.
	float blurred(vec2 p, vec2 b, float sigma) {
		vec2 k = vec2(0.70710678 / sigma);
		vec2 v = 0.5 * (erf((b + p) * k) + erf((b - p) * k));
		return v.x * v.y;
	}

	// Lucide's Disc3, the placeholder's icon, as a distance in its 24-unit grid from the centre.
	float arc(vec2 p) {
		float a = atan(p.y, p.x);
		if (a >= 0.0 && a <= 0.78539816) return abs(length(p) - 6.0);
		return min(length(p - vec2(6.0, 0.0)), length(p - vec2(4.2426407, 4.2426407)));
	}

	float disc(vec2 p) {
		float r = length(p);
		return min(min(abs(r - 10.0), abs(r - 2.0)), min(arc(p), arc(-p))) - 0.625;
	}

	vec3 art(vec2 uv) {
		// The icon is two fifths of the cover.
		vec3 placeholder = mix(u_surface, u_faint, fill(disc((uv - 0.5) * 60.0)));
		if (u_fade <= 0.0) return placeholder;
		return mix(placeholder, texture(u_art, 0.5 + (uv - 0.5) * u_crop).rgb, u_fade);
	}

	// A soft light, as the sheen on Now Playing's cover.
	vec3 lit(vec3 rgb, vec2 uv) {
		float falloff = max(0.0, 1.0 - distance(uv, u_light.xy) / 0.6);
		return rgb + (1.0 - rgb) * 0.25 * u_light.z * falloff;
	}

	void main() {
		vec2 half_ = vec2(u_cover * 0.5);
		if (u_mode == 0) {
			// As --shadow-cover: 0 10px 30px -12px, and 0 1px 2px.
			float wide = blurred(v_local - vec2(0.0, 10.0), half_ - 12.0, 15.0);
			float near = 1.0 - smoothstep(-2.0, 2.0, box(v_local - vec2(0.0, 1.0), half_, u_radius));
			o_color = vec4(0.0, 0.0, 0.0, 1.0 - (1.0 - u_shadow.x * wide) * (1.0 - u_shadow.y * near));
		} else if (u_mode == 1) {
			float d = box(v_local, half_, u_radius);
			float inner = fill(d);
			float ring = fill(d - 1.0) - inner;
			vec2 uv = v_local / u_cover + 0.5;
			o_color = vec4(lit(art(uv) * u_shade, uv), 1.0) * inner + u_ring * ring;
		} else {
			// The cover's lower part upside down, fading out.
			float t = v_local.y - half_.y - GAP;
			vec2 uv = vec2(v_local.x / u_cover + 0.5, 1.0 - t / u_cover);
			float d = box(vec2(v_local.x, t - u_reflection * 0.5), vec2(half_.x, u_reflection * 0.5), u_radius);
			float mask = 0.3 * clamp(1.0 - t / (0.85 * u_reflection), 0.0, 1.0);
			o_color = vec4(art(uv) * u_shade, 1.0) * fill(d) * mask;
		}
		o_color *= u_alpha;
	}
`;

const UNIFORMS = [
	'u_rect',
	'u_centre',
	'u_angle',
	'u_turn',
	'u_eye',
	'u_perspective',
	'u_canvas',
	'u_mode',
	'u_cover',
	'u_radius',
	'u_reflection',
	'u_art',
	'u_fade',
	'u_crop',
	'u_shade',
	'u_alpha',
	'u_ring',
	'u_surface',
	'u_faint',
	'u_shadow',
	'u_light'
] as const;

type Uniforms = Record<(typeof UNIFORMS)[number], WebGLUniformLocation | null>;

/** One cover's texture, and what is under way for it. */
interface Art {
	texture: WebGLTexture | null;
	/** Its width in pixels (an ArtSize, or an image's own width when taken from the page), and where it came from. */
	size: number;
	url: string;
	/** Its width over its height: not every cover is square, and the page crops those that aren't. */
	aspect: number;
	loading: ArtSize | 0;
	/** A size virgild doesn't have (yet): not asked for again. */
	missing: ArtSize | 0;
	/** When its first image came in, for the fade. */
	shownAt: number;
	/** The frame it was last drawn in. */
	used: number;
}

/** The theme's colours, as the CSS variables have them. */
export function readTheme(): Theme {
	const root = document.documentElement;
	const style = getComputedStyle(root);
	const read = (name: string) => cssColor(style.getPropertyValue(name));
	return { ring: read('--line'), surface: read('--surface-2'), faint: read('--faint'), dark: root.classList.contains('dark') };
}

/** `#rgb`, `#rrggbb`, `rgb(…)` or `rgba(…)`. */
export function cssColor(text: string): Rgba {
	const value = text.trim();
	const hex = hexRgb(value);
	if (hex) return [hex[0] / 255, hex[1] / 255, hex[2] / 255, 1];
	const rgb = /^rgba?\(([^)]*)\)$/i.exec(value);
	if (!rgb) return [0, 0, 0, 0];
	const [r, g, b, a = 1] = rgb[1].split(/[\s,/]+/).filter(Boolean).map(Number);
	return [r / 255, g / 255, b / 255, a];
}

/**
 * The flow's covers in sight as cards, the furthest from the middle first: on the canvas docked,
 * or on the screen with `host`, where the flow is on it.
 */
export function flowCards(scene: Scene, host: { left: number; top: number } | null): { index: number; card: Card }[] {
	const l = scene.layout;
	const { first, last } = visible(l, scene.position, scene.count);
	const eye: [number, number] = [(host?.left ?? 0) + l.width / 2, (host?.top ?? 0) + middle(l)];
	const turn = turnAngles(scene.turn);
	const [x, y] = scene.turn;
	const light: [number, number, number] = [0.5 + x * 0.4, 0.5 + y * 0.4, Math.min(1, Math.hypot(x, y) * 3)];
	const front = Math.round(scene.position);
	return backToFront(first, last, scene.position).map((index) => {
		const p = place(l, index, scene.position);
		const lift = index === front ? (scene.lift ?? 0) : 0;
		const card: Card = {
			art: scene.art(index) ?? '',
			eye,
			perspective: l.perspective,
			centre: [p.x - l.width / 2, 0, p.z + lift],
			angle: p.angle,
			turn,
			size: l.cover,
			radius: RADIUS,
			reflection: l.reflection,
			shadow: 1 + Math.max(0, lift) / l.cover,
			ring: 1,
			shade: 1 - 0.1 * Math.abs(p.d),
			opacity: 1,
			light
		};
		return { index, card };
	});
}

/** Artwork virgild serves by key, in several sizes; anything else is a full URL (radio). */
const isKey = (art: string) => !art.includes('/');

let shared: Stage | undefined;

/** The one stage, made on first use. */
export function stage(): Stage {
	return (shared ??= Stage.create());
}

export class Stage {
	readonly canvas: HTMLCanvasElement;
	#gl: WebGL2RenderingContext;
	#u: Uniforms | null = null;
	#blank: WebGLTexture | null = null;
	#anisotropy = 0;
	#theme: Theme | null = null;
	#arts = new Map<string, Art>();
	#ready: { key: string; size: ArtSize; image: HTMLImageElement }[] = [];
	#fetching = 0;
	#frames = 0;
	#raf = 0;
	#linger: ReturnType<typeof setTimeout> | undefined;
	/** The covers in sight in the flow as it left the page: kept for its coming back. */
	#kept = new Set<string>();
	#still = matchMedia('(prefers-reduced-motion: reduce)');

	/** The flow on the page, and the element it is drawn in. */
	#scene: (() => Scene) | null = null;
	#host: HTMLElement | null = null;
	/** The flow's scene as last drawn. */
	#drawn: Scene | null = null;

	static create(): Stage {
		const canvas = document.createElement('canvas');
		const gl = canvas.getContext('webgl2', { alpha: true, premultipliedAlpha: true, antialias: false, depth: false, stencil: false });
		if (!gl) throw new Error('WebGL2 is not available');
		return new Stage(canvas, gl);
	}

	private constructor(canvas: HTMLCanvasElement, gl: WebGL2RenderingContext) {
		this.canvas = canvas;
		this.#gl = gl;
		canvas.setAttribute('aria-hidden', 'true');
		canvas.className = 'pointer-events-none absolute inset-0 size-full';
		// The GPU can drop the context (memory pressure, driver reset): rebuild when it's back.
		canvas.addEventListener('webglcontextlost', (e) => {
			e.preventDefault();
			this.#u = null;
			this.#forget();
		});
		canvas.addEventListener('webglcontextrestored', () => {
			this.#setup();
			this.invalidate();
		});
		this.#setup();
	}

	/** Draw the flow, from `scene` (read on every frame), in `host`. */
	show(scene: () => Scene, host: HTMLElement) {
		clearTimeout(this.#linger);
		this.#scene = scene;
		this.#host = host;
		if (host && this.canvas.parentElement !== host) host.prepend(this.canvas);
		this.draw();
	}

	/** The flow left the page: stop drawing it, and free the textures unless something comes back soon. */
	hide() {
		const scene = this.#drawn;
		if (scene) {
			const { first, last } = visible(scene.layout, scene.position, scene.count);
			this.#kept = new Set(Array.from({ length: last - first }, (_, i) => scene.art(first + i) ?? ''));
		}
		this.canvas.remove();
		this.#scene = null;
		this.#host = null;
		this.#rest();
	}

	setTheme(theme: Theme) {
		this.#theme = theme;
		this.invalidate();
	}

	/** Redraw on the next frame. */
	invalidate() {
		if (!this.#raf && this.#scene) this.#raf = requestAnimationFrame(() => this.draw());
	}

	/** The URL of the image the flow shows for cover `art`, if it has one: for a cover flying from it. */
	source(art: string): string | null {
		const entry = this.#arts.get(art);
		return entry?.texture ? entry.url : null;
	}

	/**
	 * Takes cover `art` from an image already on the page (decoded, so this is quick), unless a
	 * texture as large is here already: a cover flying into the flow lands on its own image, not on
	 * a placeholder. Not for an image the canvas may not read (radio artwork from a host without CORS
	 * headers taints it).
	 */
	adopt(art: string, img: HTMLImageElement) {
		const entry = this.#art(art)!;
		if ((entry.texture && entry.size >= img.naturalWidth) || !img.complete || !img.naturalWidth) return;
		const gl = this.#gl;
		const texture = gl.createTexture();
		gl.bindTexture(gl.TEXTURE_2D, texture);
		try {
			gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, img);
		} catch {
			gl.deleteTexture(texture);
			return;
		}
		// On show on the page already: no fading in over the placeholder.
		this.#finish(entry, texture, img.naturalWidth, img.currentSrc, img.naturalWidth / img.naturalHeight, true);
	}

	#setup() {
		const gl = this.#gl;
		const program = gl.createProgram();
		for (const [type, source] of [
			[gl.VERTEX_SHADER, VERTEX],
			[gl.FRAGMENT_SHADER, FRAGMENT]
		] as const) {
			const shader = gl.createShader(type);
			if (!shader) return;
			gl.shaderSource(shader, source);
			gl.compileShader(shader);
			gl.attachShader(program, shader);
		}
		gl.linkProgram(program);
		if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
			console.warn('Cover shader:', gl.getProgramInfoLog(program));
			return;
		}
		gl.useProgram(program);

		gl.bindVertexArray(gl.createVertexArray());
		gl.bindBuffer(gl.ARRAY_BUFFER, gl.createBuffer());
		gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([0, 0, 1, 0, 0, 1, 1, 1]), gl.STATIC_DRAW);
		const corner = gl.getAttribLocation(program, 'a_corner');
		gl.enableVertexAttribArray(corner);
		gl.vertexAttribPointer(corner, 2, gl.FLOAT, false, 0, 0);

		this.#u = Object.fromEntries(UNIFORMS.map((name) => [name, gl.getUniformLocation(program, name)])) as Uniforms;
		gl.uniform1i(this.#u.u_art, 0);
		gl.enable(gl.BLEND);
		gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);

		// Drawn while a cover has no image yet, so the sampler always has something.
		this.#blank = gl.createTexture();
		gl.bindTexture(gl.TEXTURE_2D, this.#blank);
		gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, 1, 1, 0, gl.RGBA, gl.UNSIGNED_BYTE, new Uint8Array(4));

		const ext = gl.getExtension('EXT_texture_filter_anisotropic');
		this.#anisotropy = ext ? Math.min(8, gl.getParameter(ext.MAX_TEXTURE_MAX_ANISOTROPY_EXT)) : 0;
	}

	/** Nothing to draw: stop, and free the textures unless something comes back soon. */
	#rest() {
		if (this.#scene) return;
		cancelAnimationFrame(this.#raf);
		this.#raf = 0;
		clearTimeout(this.#linger);
		this.#linger = setTimeout(() => this.#forget(this.#kept), LINGER_MS);
	}

	/** Drops every texture but those in `keep` (they are in the HTTP cache, should they be needed again). */
	#forget(keep = new Set<string>()) {
		const gl = this.#gl;
		const lost = gl.isContextLost();
		for (const [key, art] of this.#arts) {
			if (keep.has(key) && !lost) continue;
			if (!lost) gl.deleteTexture(art.texture);
			this.#arts.delete(key);
		}
		this.#ready = [];
	}

	/** Redraw now. */
	draw() {
		cancelAnimationFrame(this.#raf);
		this.#raf = 0;
		const gl = this.#gl;
		const u = this.#u;
		const scene = this.#scene?.();
		if (!scene || !u || gl.isContextLost()) return;
		this.#drawn = scene;
		const frame = ++this.#frames;
		const now = performance.now();
		let again = this.#upload();

		const { width, height } = scene.layout;
		const dpr = Math.min(3, devicePixelRatio || 1);
		const [w, h] = [Math.round(width * dpr), Math.round(height * dpr)];
		if (this.canvas.width !== w || this.canvas.height !== h) {
			this.canvas.width = w;
			this.canvas.height = h;
		}
		gl.viewport(0, 0, w, h);
		gl.clearColor(0, 0, 0, 0);
		gl.clear(gl.COLOR_BUFFER_BIT);
		if (!width || !scene.count) return;

		// Read again only when the theme changes: on every frame, it would be a style recalculation.
		if (this.#theme?.dark !== document.documentElement.classList.contains('dark')) this.#theme = readTheme();
		const theme = this.#theme!;
		gl.uniform2f(u.u_canvas, width, height);
		gl.uniform3f(u.u_surface, theme.surface[0], theme.surface[1], theme.surface[2]);
		gl.uniform3f(u.u_faint, theme.faint[0], theme.faint[1], theme.faint[2]);
		again = this.#drawFlow(scene, frame, now, theme) || again;

		this.#evict(frame);
		if (again) this.invalidate();
	}

	/** The flow's cards (the furthest from the middle first) and the downloads for them. */
	#drawFlow(scene: Scene, frame: number, now: number, theme: Theme): boolean {
		let again = false;
		for (const { index, card } of flowCards(scene, null)) {
			const drawn = scene.drawn(index);
			if (drawn === 'none') continue;
			// The page draws that cover (and its shadow) itself; the reflection is still ours.
			if (drawn === 'reflection') card.shadow = card.ring = 0;
			again = this.#drawCard(card, frame, now, theme, drawn === 'all') || again;
		}
		const at = scene.heading ?? scene.position;
		const { first, last } = visible(scene.layout, at, scene.count);
		this.#fetch(scene, at, first, last, frame);
		return again;
	}

	/** Draws `card`: its shadow, its reflection and the cover. Returns whether it is still fading in. */
	#drawCard(card: Card, frame: number, now: number, theme: Theme, cover = true): boolean {
		const gl = this.#gl;
		const u = this.#u!;
		const art = card.art ? this.#art(card.art) : undefined;
		if (art) art.used = frame;
		const fade = art?.texture ? (this.#still.matches ? 1 : Math.min(1, (now - art.shownAt) / FADE_MS)) : 0;

		gl.uniform2f(u.u_eye, card.eye[0], card.eye[1]);
		gl.uniform1f(u.u_perspective, card.perspective);
		gl.uniform3f(u.u_centre, card.centre[0], card.centre[1], card.centre[2]);
		gl.uniform1f(u.u_angle, card.angle);
		gl.uniform2f(u.u_turn, card.turn[0], card.turn[1]);
		gl.uniform1f(u.u_cover, card.size);
		gl.uniform1f(u.u_radius, card.radius);
		gl.uniform1f(u.u_reflection, card.reflection);
		gl.uniform1f(u.u_shade, card.shade);
		gl.uniform1f(u.u_alpha, card.opacity);
		gl.uniform1f(u.u_fade, fade);
		const aspect = art?.aspect ?? 1;
		gl.uniform2f(u.u_crop, Math.min(1, 1 / aspect), Math.min(1, aspect));
		gl.uniform3f(u.u_light, card.light[0], card.light[1], card.light[2]);
		const [r, g, b, a] = theme.ring;
		const ring = a * card.ring;
		gl.uniform4f(u.u_ring, r * ring, g * ring, b * ring, ring);
		// As --shadow-cover in app.css.
		const [wide, near] = theme.dark ? [0.8, 0.5] : [0.35, 0.06];
		gl.uniform2f(u.u_shadow, Math.min(1, wide * card.shadow), Math.min(1, near * card.shadow));
		gl.bindTexture(gl.TEXTURE_2D, art?.texture ?? this.#blank);

		const h = card.size / 2;
		const pass = (mode: number, x: number, y: number, w: number, hh: number) => {
			gl.uniform1i(u.u_mode, mode);
			gl.uniform4f(u.u_rect, x, y, w, hh);
			gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
		};
		if (card.shadow > 0) pass(0, -h - 34, -h - 24, card.size + 68, card.size + 68);
		if (card.reflection > 0) pass(2, -h, h + GAP, card.size, card.reflection);
		if (cover) pass(1, -h - 2, -h - 2, card.size + 4, card.size + 4);
		return fade < 1 && !!art?.texture;
	}

	#art(key: string | null): Art | undefined {
		if (!key) return undefined;
		let art = this.#arts.get(key);
		if (!art) {
			art = { texture: null, size: 0, url: '', aspect: 1, loading: 0, missing: 0, shownAt: 0, used: 0 };
			this.#arts.set(key, art);
		}
		return art;
	}

	/** Starts downloads for the covers in sight and a few beyond, nearest the middle first. */
	#fetch(scene: Scene, position: number, first: number, last: number, frame: number) {
		const middle = Math.round(position);
		const from = Math.max(0, first - AHEAD);
		const to = Math.min(scene.count, last + AHEAD);
		const order = Array.from({ length: to - from }, (_, i) => from + i).sort(
			(a, b) => Math.abs(a - position) - Math.abs(b - position)
		);
		for (const index of order) {
			const key = scene.art(index);
			const art = this.#art(key);
			if (!key || !art) continue;
			// Loaded ahead counts as in use, or it would be the first to go.
			art.used = frame;
			if (art.loading || this.#fetching >= FETCHES) continue;
			// The middle cover at rest is as sharp as the screen can show; on a 1x screen 600 is.
			const wanted = scene.settled && index === middle && TILE > 300 ? 1000 : 600;
			// One size up at a time, from the grid's (most likely cached already).
			const next = SIZES.find((size) => size >= TILE && size > art.size && size <= wanted && size !== art.missing);
			if (next) void this.#load(key, art, next);
		}
	}

	async #load(key: string, art: Art, size: ArtSize) {
		art.loading = size;
		this.#fetching++;
		try {
			// Decoded away from the main thread. An image rather than an ImageBitmap: Safari copies a
			// decoded image to the GPU in a third of the time (a 1000 px cover: about 1 ms against 2–5),
			// and it is the same image the page shows at that size.
			const image = new Image();
			image.src = artUrl(key, size)!;
			await image.decode();
			// The page's covers can show it at once too (lib/artwork.ts).
			markDecoded(image.src);
			this.#ready.push({ key, size, image });
		} catch {
			art.missing = size;
		} finally {
			art.loading = 0;
			this.#fetching--;
			this.invalidate();
		}
	}

	/** Moves decoded images to the GPU, a few per frame. Returns whether more are waiting. */
	#upload(): boolean {
		const gl = this.#gl;
		for (let i = 0; i < UPLOADS && this.#ready.length; i++) {
			const { key, size, image } = this.#ready.shift()!;
			const art = this.#arts.get(key);
			if (!art || size <= art.size) continue;
			const texture = gl.createTexture();
			gl.bindTexture(gl.TEXTURE_2D, texture);
			gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, image);
			this.#finish(art, texture, size, image.src, image.naturalWidth / image.naturalHeight);
		}
		return this.#ready.length > 0;
	}

	/** Mipmaps the texture just uploaded for `art` and puts it in place of the old one. A first one fades in, unless `shown`. */
	#finish(art: Art, texture: WebGLTexture | null, size: number, url: string, aspect: number, shown = false) {
		const gl = this.#gl;
		// Mipmaps (and anisotropy) keep the turned covers from shimmering.
		gl.generateMipmap(gl.TEXTURE_2D);
		gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR_MIPMAP_LINEAR);
		gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
		gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
		gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
		if (this.#anisotropy) gl.texParameterf(gl.TEXTURE_2D, 0x84fe /* TEXTURE_MAX_ANISOTROPY_EXT */, this.#anisotropy);
		if (art.texture) gl.deleteTexture(art.texture);
		else art.shownAt = shown ? -Infinity : performance.now();
		art.texture = texture;
		art.size = size;
		art.url = url;
		art.aspect = aspect;
	}

	/** Keeps the GPU to `KEEP` covers, dropping those drawn longest ago (never this frame's). */
	#evict(frame: number) {
		if (this.#arts.size <= KEEP) return;
		const old = [...this.#arts].filter(([, art]) => art.used < frame && !art.loading).sort(([, a], [, b]) => a.used - b.used);
		for (const [key, art] of old.slice(0, this.#arts.size - KEEP)) {
			this.#gl.deleteTexture(art.texture);
			this.#arts.delete(key);
		}
	}
}
