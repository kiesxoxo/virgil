// Draws the cover flow with WebGL2: every cover a quad seen through one camera, with its shadow
// and reflection, all in one canvas, so scrolling costs the page nothing but a redraw. Where
// things go comes from coverflow.ts. The renderer and its canvas outlive the flow on the page:
// coming back to it (and a page transition's picture of it) finds the covers already loaded.

import { GAP, backToFront, middle, place, visible, type Layout } from './coverflow.ts';
import { TILE, artUrl, type ArtSize } from './format.ts';
import { hexRgb } from './palette.ts';

export interface Scene {
	layout: Layout;
	/** Scroll position, in covers. */
	position: number;
	count: number;
	/** Cover `index`'s artwork key. */
	art(index: number): string | null;
	/** A cover the page draws itself (the one a page transition moves), or -1. */
	hidden: number;
	/** At rest on a cover: the middle one gets the sharpest image. */
	settled: boolean;
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
/** Textures uploaded per frame: each is a decoded image copied to the GPU, plus its mipmaps. */
const UPLOADS = 2;
/** Covers kept on the GPU, the least recently drawn going first. */
const KEEP = 32;
/** Covers beyond those in sight to load ahead, on each side. */
const AHEAD = 2;
/** An image fading in over its placeholder. */
const FADE_MS = 200;
/** How long textures stay after the flow leaves the page, for coming back to it. */
const LINGER_MS = 30_000;
const SIZES: ArtSize[] = [300, 600, 1000];

const VERTEX = `#version 300 es
	in vec2 a_corner;
	uniform vec4 u_rect;  // the quad, from the cover's centre: x, y, width, height
	uniform vec3 u_place; // the cover's centre across the flow, its depth and its turn
	uniform vec4 u_view;  // the flow's width and height, the covers' middle, the camera's distance
	out vec2 v_local;

	void main() {
		vec2 local = u_rect.xy + a_corner * u_rect.zw;
		float c = cos(u_place.z), s = sin(u_place.z);
		vec3 p = vec3(u_place.x + local.x * c, u_view.z + local.y, u_place.y - local.x * s);
		// Perspective as CSS does it, divided by the GPU so textures stay true across the quad.
		float w = (u_view.w - p.z) / u_view.w;
		vec2 eye = vec2(u_view.x * 0.5, u_view.z);
		vec2 screen = eye * w + p.xy - eye;
		gl_Position = vec4(screen.x / u_view.x * 2.0 - w, w - screen.y / u_view.y * 2.0, 0.0, w);
		v_local = local;
	}
`;

const FRAGMENT = `#version 300 es
	precision highp float;

	in vec2 v_local;
	out vec4 o_color;

	uniform int u_mode; // 0 shadow, 1 cover, 2 reflection
	uniform float u_cover;
	uniform float u_reflection;
	uniform sampler2D u_art;
	uniform float u_fade;   // 0 placeholder … 1 artwork
	uniform float u_shade;
	uniform vec4 u_ring;    // premultiplied
	uniform vec3 u_surface;
	uniform vec3 u_faint;
	uniform vec2 u_shadow;  // opacity of the wide and the close shadow

	const float RADIUS = 12.0;
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
		return mix(placeholder, texture(u_art, uv).rgb, u_fade);
	}

	void main() {
		vec2 half_ = vec2(u_cover * 0.5);
		if (u_mode == 0) {
			// As --shadow-cover: 0 10px 30px -12px, and 0 1px 2px.
			float wide = blurred(v_local - vec2(0.0, 10.0), half_ - 12.0, 15.0);
			float near = 1.0 - smoothstep(-2.0, 2.0, box(v_local - vec2(0.0, 1.0), half_, RADIUS));
			o_color = vec4(0.0, 0.0, 0.0, 1.0 - (1.0 - u_shadow.x * wide) * (1.0 - u_shadow.y * near));
		} else if (u_mode == 1) {
			float d = box(v_local, half_, RADIUS);
			float inner = fill(d);
			float ring = fill(d - 1.0) - inner;
			o_color = vec4(art(v_local / u_cover + 0.5) * u_shade, 1.0) * inner + u_ring * ring;
		} else {
			// The cover's lower part upside down, fading out.
			float t = v_local.y - half_.y - GAP;
			vec2 uv = vec2(v_local.x / u_cover + 0.5, 1.0 - t / u_cover);
			float d = box(vec2(v_local.x, t - u_reflection * 0.5), vec2(half_.x, u_reflection * 0.5), RADIUS);
			float mask = 0.3 * clamp(1.0 - t / (0.85 * u_reflection), 0.0, 1.0);
			o_color = vec4(art(uv) * u_shade, 1.0) * fill(d) * mask;
		}
	}
`;

const UNIFORMS = [
	'u_rect',
	'u_place',
	'u_view',
	'u_mode',
	'u_cover',
	'u_reflection',
	'u_art',
	'u_fade',
	'u_shade',
	'u_ring',
	'u_surface',
	'u_faint',
	'u_shadow'
] as const;

type Uniforms = Record<(typeof UNIFORMS)[number], WebGLUniformLocation | null>;

/** One album cover's texture, and what is under way for it. */
interface Art {
	texture: WebGLTexture | null;
	size: ArtSize | 0;
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

/** Whether this browser has WebGL2 at all (the flow needs it). */
export const hasWebGL2 = typeof WebGL2RenderingContext === 'function';

let shared: FlowRenderer | null | undefined;

/** The one renderer, made on first use; null when WebGL2 can't be had. */
export function flowRenderer(): FlowRenderer | null {
	if (shared === undefined) shared = hasWebGL2 ? FlowRenderer.create() : null;
	return shared;
}

export class FlowRenderer {
	readonly canvas: HTMLCanvasElement;
	#gl: WebGL2RenderingContext;
	#u: Uniforms | null = null;
	#blank: WebGLTexture | null = null;
	#anisotropy = 0;
	#theme: Theme | null = null;
	#scene: (() => Scene) | null = null;
	#arts = new Map<string, Art>();
	#ready: { key: string; size: ArtSize; image: ImageBitmap }[] = [];
	#fetching = 0;
	#frames = 0;
	#raf = 0;
	#linger: ReturnType<typeof setTimeout> | undefined;
	#still = matchMedia('(prefers-reduced-motion: reduce)');

	static create(): FlowRenderer | null {
		const canvas = document.createElement('canvas');
		const gl = canvas.getContext('webgl2', { alpha: true, premultipliedAlpha: true, antialias: false, depth: false, stencil: false });
		if (!gl) return null;
		const renderer = new FlowRenderer(canvas, gl);
		return renderer.#u ? renderer : null;
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

	/** Draw `scene` from now on (it is read on every frame). */
	show(scene: () => Scene) {
		clearTimeout(this.#linger);
		this.#scene = scene;
		this.draw();
	}

	/** The flow left the page: stop drawing, and free the textures unless it comes back soon. */
	hide() {
		this.#scene = null;
		cancelAnimationFrame(this.#raf);
		this.#raf = 0;
		this.#linger = setTimeout(() => this.#forget(), LINGER_MS);
	}

	setTheme(theme: Theme) {
		this.#theme = theme;
		this.invalidate();
	}

	/** Redraw on the next frame. */
	invalidate() {
		if (!this.#raf && this.#scene) this.#raf = requestAnimationFrame(() => this.draw());
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
			console.warn('Cover flow shader:', gl.getProgramInfoLog(program));
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

	/** Drops every texture (they are in the HTTP cache, should they be needed again). */
	#forget() {
		const gl = this.#gl;
		if (!gl.isContextLost()) for (const art of this.#arts.values()) gl.deleteTexture(art.texture);
		this.#arts.clear();
		for (const { image } of this.#ready) image.close();
		this.#ready = [];
	}

	/** Redraw now. */
	draw() {
		cancelAnimationFrame(this.#raf);
		this.#raf = 0;
		const gl = this.#gl;
		const u = this.#u;
		const scene = this.#scene?.();
		if (!u || !scene || gl.isContextLost()) return;
		const l = scene.layout;
		const frame = ++this.#frames;
		let again = this.#upload();

		const dpr = Math.min(3, devicePixelRatio || 1);
		const [width, height] = [Math.round(l.width * dpr), Math.round(l.height * dpr)];
		if (this.canvas.width !== width || this.canvas.height !== height) {
			this.canvas.width = width;
			this.canvas.height = height;
		}
		gl.viewport(0, 0, width, height);
		gl.clearColor(0, 0, 0, 0);
		gl.clear(gl.COLOR_BUFFER_BIT);
		if (!l.width || !scene.count) return;

		const theme = this.#theme ?? readTheme();
		gl.uniform4f(u.u_view, l.width, l.height, middle(l), l.perspective);
		gl.uniform1f(u.u_cover, l.cover);
		gl.uniform1f(u.u_reflection, l.reflection);
		const [r, g, b, a] = theme.ring;
		gl.uniform4f(u.u_ring, r * a, g * a, b * a, a);
		gl.uniform3f(u.u_surface, theme.surface[0], theme.surface[1], theme.surface[2]);
		gl.uniform3f(u.u_faint, theme.faint[0], theme.faint[1], theme.faint[2]);
		// As --shadow-cover in app.css.
		if (theme.dark) gl.uniform2f(u.u_shadow, 0.8, 0.5);
		else gl.uniform2f(u.u_shadow, 0.35, 0.06);

		const { first, last } = visible(l, scene.position, scene.count);
		const h = l.cover / 2;
		const now = performance.now();
		for (const index of backToFront(first, last, scene.position)) {
			const p = place(l, index, scene.position);
			const art = this.#art(scene.art(index));
			if (art) art.used = frame;
			const fade = art?.texture ? (this.#still.matches ? 1 : Math.min(1, (now - art.shownAt) / FADE_MS)) : 0;
			if (fade < 1 && art?.texture) again = true;

			gl.uniform3f(u.u_place, p.x, p.z, p.angle);
			gl.uniform1f(u.u_shade, 1 - 0.1 * Math.abs(p.d));
			gl.uniform1f(u.u_fade, fade);
			gl.bindTexture(gl.TEXTURE_2D, art?.texture ?? this.#blank);
			const pass = (mode: number, x: number, y: number, w: number, hh: number) => {
				gl.uniform1i(u.u_mode, mode);
				gl.uniform4f(u.u_rect, x, y, w, hh);
				gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
			};
			// The page draws the hidden cover (and its shadow) itself; the reflection is still ours.
			if (index !== scene.hidden) pass(0, -h - 34, -h - 24, l.cover + 68, l.cover + 68);
			pass(2, -h, h + GAP, l.cover, l.reflection);
			if (index !== scene.hidden) pass(1, -h - 2, -h - 2, l.cover + 4, l.cover + 4);
		}

		this.#fetch(scene, first, last, frame);
		this.#evict(frame);
		if (again) this.invalidate();
	}

	#art(key: string | null): Art | undefined {
		if (!key) return undefined;
		let art = this.#arts.get(key);
		if (!art) {
			art = { texture: null, size: 0, loading: 0, missing: 0, shownAt: 0, used: 0 };
			this.#arts.set(key, art);
		}
		return art;
	}

	/** Starts downloads for the covers in sight and a few beyond, nearest the middle first. */
	#fetch(scene: Scene, first: number, last: number, frame: number) {
		const middle = Math.round(scene.position);
		const from = Math.max(0, first - AHEAD);
		const to = Math.min(scene.count, last + AHEAD);
		const order = Array.from({ length: to - from }, (_, i) => from + i).sort(
			(a, b) => Math.abs(a - scene.position) - Math.abs(b - scene.position)
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
			if (next) this.#load(key, art, next);
		}
	}

	async #load(key: string, art: Art, size: ArtSize) {
		art.loading = size;
		this.#fetching++;
		try {
			const res = await fetch(artUrl(key, size)!);
			if (!res.ok) throw new Error(res.statusText);
			// Decoded away from the main thread.
			const image = await createImageBitmap(await res.blob());
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
			if (!art || size <= art.size) {
				image.close();
				continue;
			}
			const texture = gl.createTexture();
			gl.bindTexture(gl.TEXTURE_2D, texture);
			gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, image);
			image.close();
			// Mipmaps (and anisotropy) keep the turned covers from shimmering.
			gl.generateMipmap(gl.TEXTURE_2D);
			gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR_MIPMAP_LINEAR);
			gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
			gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
			gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
			if (this.#anisotropy) gl.texParameterf(gl.TEXTURE_2D, 0x84fe /* TEXTURE_MAX_ANISOTROPY_EXT */, this.#anisotropy);
			if (art.texture) gl.deleteTexture(art.texture);
			else art.shownAt = performance.now();
			art.texture = texture;
			art.size = size;
		}
		return this.#ready.length > 0;
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
