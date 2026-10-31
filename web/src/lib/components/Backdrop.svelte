<script lang="ts">
	import { untrack } from 'svelte';
	import { hexRgb, mix, oklab, palette, saturate, spread, type Rgb } from '../palette.ts';

	/** Soft fields in the cover's colours, drifting slowly while `playing` and gliding to a stop on pause. */
	let { src, playing }: { src: string | null; playing: boolean } = $props();

	/**
	 * Both painters draw small and let the browser stretch the canvas over the screen: that is all
	 * the blur the fields need (a CSS blur would filter the whole screen every frame).
	 */
	const CSS_PX_PER_PIXEL = 4;
	const MAX_PIXELS = 256;
	/** The canvas 2D fallback's size. */
	const SIZE = 64;
	const FRAME_MS = 1000 / 30;
	/** Seconds to blend into the next cover's colours. */
	const FADE = 1.5;
	/** Colours a little richer than the cover's, so they hold up under the fade to the background. */
	const SATURATE = 1.5;
	/** Periods in seconds, far from multiples of each other so the pattern never visibly repeats. */
	const FIELDS = [
		{ x: [23, 41], y: [29, 37], r: 19, phase: 0 },
		{ x: [31, 43], y: [26, 47], r: 23, phase: 1.3 },
		{ x: [37, 29], y: [41, 22], r: 17, phase: 2.6 },
		{ x: [27, 47], y: [33, 39], r: 29, phase: 3.9 },
		{ x: [43, 25], y: [21, 31], r: 13, phase: 5.2 }
	];

	/** Where each field is at time `t`: centre in 0–1 of the canvas, radius in its shorter side. */
	type Field = { x: number; y: number; r: number; color: Rgb };
	type Painter = (fields: Field[], t: number) => void;

	/** The cover's colours once loaded; null for no cover (or one the canvas may not read). */
	let colors = $state.raw<Rgb[] | null>(null);

	$effect(() => {
		if (!src) {
			colors = null;
			return;
		}
		let current = true;
		const img = new Image();
		// Same-origin covers always work; radio artwork only if its host sends CORS headers.
		img.crossOrigin = 'anonymous';
		img.onload = () => current && (colors = sample(img));
		img.onerror = () => current && (colors = null);
		img.src = src;
		return () => (current = false);
	});

	function sample(img: HTMLImageElement): Rgb[] | null {
		const canvas = document.createElement('canvas');
		canvas.width = canvas.height = 24;
		const ctx = canvas.getContext('2d', { willReadFrequently: true });
		if (!ctx) return null;
		ctx.drawImage(img, 0, 0, 24, 24);
		try {
			return palette(ctx.getImageData(0, 0, 24, 24).data).map((c) => saturate(c, SATURATE));
		} catch {
			return null; // tainted
		}
	}

	function drift(canvas: HTMLCanvasElement) {
		let paint = webgl(canvas) ?? canvas2d(canvas);
		// A canvas keeps its first context: after a WebGL that failed past getContext, no 2D either.
		if (!paint) return void (canvas.hidden = true);
		const still = matchMedia('(prefers-reduced-motion: reduce)');
		const accent = hexRgb(getComputedStyle(document.documentElement).getPropertyValue('--accent'));
		const fallback = spread([accent ?? [236, 74, 47]], 4).map((c) => saturate(c, SATURATE));

		// Read untracked: the loop picks up changes itself instead of starting over.
		let target = untrack(() => colors) ?? fallback;
		let from = target;
		let shown = target;
		let blend = 1;
		let t = Math.random() * 600;
		let speed = untrack(() => playing) && !still.matches ? 1 : 0;
		let last = 0;
		let dirty = true;

		// The GPU can drop the context (driver reset, too many contexts): wait for it to come back.
		const lost = (e: Event) => {
			e.preventDefault();
			paint = null;
		};
		const restored = () => {
			paint = webgl(canvas);
			dirty = true;
		};
		canvas.addEventListener('webglcontextlost', lost);
		canvas.addEventListener('webglcontextrestored', restored);

		// rAF also stops by itself while the tab is hidden.
		let raf = requestAnimationFrame(function frame(now) {
			raf = requestAnimationFrame(frame);
			if (last && now - last < FRAME_MS - 1) return;
			const dt = last ? Math.min(0.1, (now - last) / 1000) : 0;
			last = now;

			const wanted = colors ?? fallback;
			if (wanted !== target) {
				[from, target, blend] = [shown, wanted, 0];
			}
			if (blend < 1) {
				blend = still.matches ? 1 : Math.min(1, blend + dt / FADE);
				const k = blend * blend * (3 - 2 * blend);
				shown = target.map((c, i) => mix(from[i % from.length], c, k));
				dirty = true;
			}

			const goal = playing && !still.matches ? 1 : 0;
			speed += (goal - speed) * Math.min(1, dt * 3);
			if (Math.abs(goal - speed) < 0.001) speed = goal;
			if (speed) {
				t += dt * speed;
				dirty = true;
			}

			if (!dirty || !paint) return;
			dirty = false;
			paint(place(shown, t), t);
		});
		return () => {
			cancelAnimationFrame(raf);
			canvas.removeEventListener('webglcontextlost', lost);
			canvas.removeEventListener('webglcontextrestored', restored);
		};
	}

	function place(shown: Rgb[], t: number): Field[] {
		const wave = (period: number, phase: number) => Math.sin((t * 2 * Math.PI) / period + phase);
		return FIELDS.map((f, i) => ({
			x: 0.5 + 0.3 * wave(f.x[0], f.phase) + 0.12 * wave(f.x[1], f.phase * 1.7),
			y: 0.5 + 0.3 * wave(f.y[0], f.phase + 0.8) + 0.12 * wave(f.y[1], f.phase * 2.3),
			// Radius (and, in the shader, the warp) is where the music's energy will come in.
			r: 0.5 * (1 + 0.08 * wave(f.r, f.phase)),
			color: shown[i % shown.length]
		}));
	}

	const VERTEX = `
		attribute vec2 a_position;
		void main() { gl_Position = vec4(a_position, 0.0, 1.0); }
	`;

	const FRAGMENT = `
		#ifdef GL_FRAGMENT_PRECISION_HIGH
		precision highp float;
		#else
		precision mediump float;
		#endif

		uniform vec2 u_resolution;
		uniform float u_time;
		uniform vec2 u_points[5];
		uniform float u_radii[5];
		uniform vec3 u_colors[5]; // OKLab

		float hash(vec2 p) {
			return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453);
		}

		float noise(vec2 p) {
			vec2 i = floor(p);
			vec2 f = fract(p);
			vec2 u = f * f * (3.0 - 2.0 * f);
			return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), u.x), mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), u.x), u.y);
		}

		vec3 srgb(vec3 lab) {
			float l = lab.x + 0.3963377774 * lab.y + 0.2158037573 * lab.z;
			float m = lab.x - 0.1055613458 * lab.y - 0.0638541728 * lab.z;
			float s = lab.x - 0.0894841775 * lab.y - 1.2914855480 * lab.z;
			l = l * l * l;
			m = m * m * m;
			s = s * s * s;
			vec3 linear = clamp(vec3(
				4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
				-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
				-0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s
			), 0.0, 1.0);
			return mix(linear * 12.92, 1.055 * pow(linear, vec3(1.0 / 2.4)) - 0.055, step(0.0031308, linear));
		}

		void main() {
			vec2 uv = vec2(gl_FragCoord.x, u_resolution.y - gl_FragCoord.y) / u_resolution;
			// Slow noise bends the blends between fields into flowing shapes.
			vec2 q = uv * 2.0;
			uv += 0.24 * (vec2(noise(q + vec2(0.0, u_time * 0.05)), noise(q + vec2(5.2, 1.3) - u_time * 0.04)) - 0.5);

			vec2 aspect = u_resolution / min(u_resolution.x, u_resolution.y);
			vec3 sum = vec3(0.0);
			float total = 0.0;
			for (int i = 0; i < 5; i++) {
				vec2 d = (uv - u_points[i]) * aspect;
				float w = exp(-dot(d, d) / (u_radii[i] * u_radii[i]));
				sum += u_colors[i] * w;
				total += w;
			}
			vec3 color = srgb(sum / max(total, 1e-4));
			// A pixel's worth of noise keeps the smooth gradients from banding.
			gl_FragColor = vec4(color + (hash(gl_FragCoord.xy) - 0.5) / 255.0, 1.0);
		}
	`;

	function webgl(canvas: HTMLCanvasElement): Painter | null {
		const gl = canvas.getContext('webgl', { alpha: false, antialias: false, depth: false, stencil: false, powerPreference: 'low-power' });
		if (!gl) return null;
		const program = gl.createProgram();
		for (const [type, source] of [
			[gl.VERTEX_SHADER, VERTEX],
			[gl.FRAGMENT_SHADER, FRAGMENT]
		] as const) {
			const shader = gl.createShader(type);
			if (!shader) return null;
			gl.shaderSource(shader, source);
			gl.compileShader(shader);
			gl.attachShader(program, shader);
		}
		gl.linkProgram(program);
		if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
			console.warn('Backdrop shader:', gl.getProgramInfoLog(program));
			return null;
		}
		gl.useProgram(program);
		// One triangle that covers the whole viewport.
		gl.bindBuffer(gl.ARRAY_BUFFER, gl.createBuffer());
		gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);
		const position = gl.getAttribLocation(program, 'a_position');
		gl.enableVertexAttribArray(position);
		gl.vertexAttribPointer(position, 2, gl.FLOAT, false, 0, 0);
		const uniform = (name: string) => gl.getUniformLocation(program, name);
		const [resolution, time, points, radii, labs] = ['u_resolution', 'u_time', 'u_points', 'u_radii', 'u_colors'].map(uniform);

		return (fields, t) => {
			const scale = Math.max(CSS_PX_PER_PIXEL, Math.max(canvas.clientWidth, canvas.clientHeight) / MAX_PIXELS);
			const [width, height] = [Math.ceil(canvas.clientWidth / scale) || 1, Math.ceil(canvas.clientHeight / scale) || 1];
			if (canvas.width !== width || canvas.height !== height) {
				[canvas.width, canvas.height] = [width, height];
				gl.viewport(0, 0, width, height);
			}
			gl.uniform2f(resolution, width, height);
			gl.uniform1f(time, t);
			gl.uniform2fv(points, fields.flatMap((f) => [f.x, f.y]));
			gl.uniform1fv(radii, fields.map((f) => f.r));
			gl.uniform3fv(labs, fields.flatMap((f) => oklab(f.color)));
			gl.drawArrays(gl.TRIANGLES, 0, 3);
		};
	}

	function canvas2d(canvas: HTMLCanvasElement): Painter | null {
		const ctx = canvas.getContext('2d');
		if (!ctx) return null;
		canvas.width = canvas.height = SIZE;
		return (fields) => {
			ctx.fillStyle = rgba(darkest(fields.map((f) => f.color)), 1);
			ctx.fillRect(0, 0, SIZE, SIZE);
			for (const { x, y, r, color } of fields) {
				const gradient = ctx.createRadialGradient(x * SIZE, y * SIZE, 0, x * SIZE, y * SIZE, r * SIZE);
				gradient.addColorStop(0, rgba(color, 0.9));
				gradient.addColorStop(1, rgba(color, 0));
				ctx.fillStyle = gradient;
				ctx.fillRect(0, 0, SIZE, SIZE);
			}
		};
	}

	function darkest(shown: Rgb[]): Rgb {
		const luma = ([r, g, b]: Rgb) => 0.2126 * r + 0.7152 * g + 0.0722 * b;
		return shown.reduce((a, b) => (luma(b) < luma(a) ? b : a));
	}

	function rgba([r, g, b]: Rgb, a: number): string {
		return `rgb(${r} ${g} ${b} / ${a})`;
	}
</script>

<canvas aria-hidden="true" class="size-full scale-150 opacity-50 dark:opacity-40" {@attach drift}></canvas>
