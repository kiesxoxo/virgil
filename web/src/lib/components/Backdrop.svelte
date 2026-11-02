<script lang="ts">
	import { ambient, colorsOf, type Claim } from '../ambient.svelte.ts';
	import { hexRgb, mix, oklab, spread, saturate, type Rgb } from '../palette.ts';
	import { theme } from '../theme.svelte.ts';

	/**
	 * The colour behind every page: soft fields in a cover's colours, drifting slowly while the
	 * page asks for it (Now Playing, while playing) and gliding to a stop when it doesn't. What
	 * each page wants comes from lib/ambient.svelte.ts; from one page to the next the colours and
	 * their strength blend, or grow out of (and shrink back into) the mini player.
	 *
	 * It draws small and lets the browser stretch the canvas over the screen: that is all
	 * the blur the fields need (a CSS blur would filter the whole screen every frame).
	 */
	const CSS_PX_PER_PIXEL = 4;
	const MAX_PIXELS = 256;
	const FRAME_MS = 1000 / 30;
	/** Seconds to blend into the next cover's colours. */
	const FADE = 1.5;
	/** Seconds for the colour to come in or go from one page to the next. */
	const REVEAL = 0.6;
	/** The canvas is drawn this much larger than the screen (scale-150), so the fields' edges stay out of sight. */
	const SCALE = 1.5;
	/** The soft edge of a growing or shrinking circle of colour, in CSS pixels. */
	const SOFT = 120;
	/** Colours a little richer than the accent, so they hold up under the fade to the background. */
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
	/** How much colour shows (0–1) and how far down the screen it reaches (0–1). */
	type Layer = { strength: number; reach: number };
	/**
	 * What to draw over the background: `outer` turning into `inner`, everywhere at once as `mix`
	 * goes from 0 to 1, or inside a circle (CSS pixels: centre, radius) when there is one.
	 */
	type Look = { inner: Layer; outer: Layer; circle: [number, number, number] | null; mix: number; bg: Rgb; opacity: number };
	/** `size`: the canvas's, in CSS pixels. */
	type Painter = (fields: Field[], t: number, look: Look, size: [number, number]) => void;

	const NONE: Layer = { strength: 0, reach: 1 };
	const lerp = (a: Layer, b: Layer, k: number): Layer => ({
		strength: a.strength + (b.strength - a.strength) * k,
		reach: a.reach + (b.reach - a.reach) * k
	});
	const ease = (p: number) => 1 - (1 - p) ** 3;

	/** The colours of the image the page asked for; null for none (the accent's). Undefined: no page asks. */
	let src = $derived(ambient.claim ? ambient.claim.src : undefined);
	let colors = $state.raw<Rgb[] | null>(null);
	/** The image `colors` are from: until it is the page's, its colours are still coming. */
	let colorsFrom: string | null | undefined = undefined;
	$effect(() => {
		if (src === undefined) return; // keeps the last page's colours while they fade
		const from = src;
		if (from === null) return void ([colors, colorsFrom] = [null, null]);
		let current = true;
		colorsOf(from).then((c) => current && ([colors, colorsFrom] = [c, from]));
		return () => (current = false);
	});

	/** Starts drawing again (after a change, when it has stopped). */
	let wake = () => {};
	$effect(() => {
		void [ambient.claim, colors, theme.dark];
		wake();
	});

	function drift(canvas: HTMLCanvasElement) {
		let paint = webgl(canvas);
		if (!paint) return void (canvas.hidden = true);
		const still = matchMedia('(prefers-reduced-motion: reduce)');
		const style = () => getComputedStyle(document.documentElement);
		const accent = hexRgb(style().getPropertyValue('--accent'));
		const fallback = spread([accent ?? [236, 74, 47]], 4).map((c) => saturate(c, SATURATE));
		// Read once, not on every frame: each read is a style recalculation (and the size a layout),
		// in the middle of a page coming in.
		let background: { dark: boolean; rgb: Rgb } | null = null;
		let size: [number, number] = [canvas.clientWidth, canvas.clientHeight];

		let target = fallback;
		let from = target;
		let shown = target;
		let blend = 1;
		let t = Math.random() * 600;
		let speed = 0;
		let last = 0;
		let raf = 0;

		// The page shown, and the transition to it.
		let key: string | null = null;
		let inner = NONE;
		let outer = NONE;
		let circle: { x: number; y: number; grow: boolean } | null = null;
		let progress = 1;
		/** Where the page's colour grew from: it shrinks back there when the page goes. */
		let shrinkTo: { x: number; y: number } | null = null;

		/** Roughly what shows now, to start the next transition from. */
		const now = (): Layer => {
			if (progress >= 1) return inner;
			const k = ease(progress);
			return circle && !circle.grow ? lerp(inner, outer, k) : lerp(outer, inner, k);
		};

		const follow = (claim: Claim | null) => {
			const next = claim ? { strength: claim.strength, reach: claim.reach } : { ...NONE, reach: now().reach };
			if ((claim?.key ?? null) === key) {
				// The same page: follow it at once (the album page's colour fading as it scrolls).
				if (circle && !circle.grow) outer = next;
				else inner = next;
				return;
			}
			const was = now();
			if (claim?.from) [inner, outer, circle] = [next, was, { ...claim.from, grow: true }];
			else if (shrinkTo) [inner, outer, circle] = [was, next, { ...shrinkTo, grow: false }];
			else [inner, outer, circle] = [next, was, null];
			progress = still.matches ? 1 : 0;
			key = claim?.key ?? null;
			shrinkTo = claim?.from ?? null;
		};

		const look = (): Look => {
			if (background?.dark !== theme.dark) background = { dark: theme.dark, rgb: hexRgb(style().getPropertyValue('--bg')) ?? [255, 255, 255] };
			const bg = background.rgb;
			const opacity = theme.dark ? 0.4 : 0.5;
			const k = ease(progress);
			if (!circle) return { inner, outer, circle: null, mix: k, bg, opacity };
			const [w, h] = [innerWidth, innerHeight];
			const far = Math.hypot(Math.max(circle.x, w - circle.x), Math.max(circle.y, h - circle.y)) + SOFT;
			return { inner, outer, circle: [circle.x, circle.y, (circle.grow ? k : 1 - k) * far], mix: 1, bg, opacity };
		};

		/** When a new page's colours started to be awaited. */
		let waiting = 0;

		const frame = (time: number) => {
			raf = 0;
			const claim = ambient.claim;
			// A new page: wait for its colours (they are usually cached) rather than start in the old ones.
			const ready = !claim || colorsFrom === claim.src;
			if (!ready && (claim?.key ?? null) !== key) {
				waiting ||= time;
				if (time - waiting < 300) return void (raf = requestAnimationFrame(frame));
			}
			waiting = 0;
			const before = now().strength;
			const drifting = !!claim?.drift && !still.matches;
			// Only drifting runs at 30 fps; a page's colour coming or going gets every frame.
			if (last && progress >= 1 && blend >= 1 && time - last < FRAME_MS - 1) return void (raf = requestAnimationFrame(frame));
			const dt = last ? Math.min(0.1, (time - last) / 1000) : 0;
			last = time;

			follow(claim);
			if (progress < 1) progress = still.matches ? 1 : Math.min(1, progress + dt / REVEAL);
			if (progress >= 1) {
				// Settled: one layer, everywhere.
				if (circle && !circle.grow) inner = outer;
				outer = inner;
				circle = null;
			}

			// Without a page asking (or while its colours are coming), the last colours stay.
			const wanted = claim && ready ? (colors ?? fallback) : target;
			if (wanted !== target) {
				[from, target, blend] = [shown, wanted, 0];
				// Nothing was showing: nothing to blend from.
				if (before < 0.001) [from, shown, blend] = [wanted, wanted, 1];
			}
			if (blend < 1) {
				blend = still.matches ? 1 : Math.min(1, blend + dt / FADE);
				const k = blend * blend * (3 - 2 * blend);
				shown = target.map((c, i) => mix(from[i % from.length], c, k));
			}

			const goal = drifting ? 1 : 0;
			speed += (goal - speed) * Math.min(1, dt * 3);
			if (Math.abs(goal - speed) < 0.001) speed = goal;
			t += dt * speed;

			const visible = progress < 1 || inner.strength > 0.001;
			const visibility = visible ? '' : 'hidden';
			if (canvas.style.visibility !== visibility) canvas.style.visibility = visibility;
			if (!visible || !paint) return void (last = 0);
			paint(place(shown, t), t, look(), size);
			if (progress < 1 || blend < 1 || speed > 0) raf = requestAnimationFrame(frame);
			else last = 0;
		};

		wake = () => {
			if (!raf) raf = requestAnimationFrame(frame);
		};
		wake();

		// The GPU can drop the context (driver reset, too many contexts): wait for it to come back.
		const lost = (e: Event) => {
			e.preventDefault();
			paint = null;
		};
		const restored = () => {
			paint = webgl(canvas);
			wake();
		};
		canvas.addEventListener('webglcontextlost', lost);
		canvas.addEventListener('webglcontextrestored', restored);
		const resized = new ResizeObserver(([entry]) => {
			size = [entry.contentRect.width, entry.contentRect.height];
			wake();
		});
		resized.observe(canvas);
		return () => {
			cancelAnimationFrame(raf);
			wake = () => {};
			canvas.removeEventListener('webglcontextlost', lost);
			canvas.removeEventListener('webglcontextrestored', restored);
			resized.disconnect();
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
		uniform vec2 u_screen;    // the screen, in CSS pixels
		uniform vec2 u_inner;     // a layer: strength and reach (see Look)
		uniform vec2 u_outer;
		uniform vec3 u_circle;    // centre and radius in CSS pixels; radius < 0 for none
		uniform float u_mix;
		uniform vec3 u_bg;
		uniform float u_opacity;

		const float SCALE = ${SCALE.toFixed(2)};
		const float SOFT = ${SOFT.toFixed(1)};

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

		// How much of a layer's colour shows at height y (0 top … 1 bottom of the screen): most at
		// the top, fading out by its reach.
		float weight(vec2 layer, float y) {
			float t = 2.0 * y / max(layer.y, 1e-3);
			return layer.x * (t < 1.0 ? mix(0.7, 0.4, t) : mix(0.4, 0.0, clamp(t - 1.0, 0.0, 1.0)));
		}

		void main() {
			vec2 uv = vec2(gl_FragCoord.x, u_resolution.y - gl_FragCoord.y) / u_resolution;
			// Where this is on the screen: the canvas is larger, centred.
			vec2 screen = (0.5 + (uv - 0.5) * SCALE) * u_screen;
			float inside = u_circle.z < 0.0 ? u_mix : 1.0 - smoothstep(u_circle.z - SOFT, u_circle.z, distance(screen, u_circle.xy));
			float y = screen.y / u_screen.y;
			float w = mix(weight(u_outer, y), weight(u_inner, y), inside) * u_opacity;

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
			vec3 color = mix(u_bg, srgb(sum / max(total, 1e-4)), w);
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
		const [resolution, time, points, radii, labs, screen, innerLayer, outerLayer, circle, blend, bg, opacity] = [
			'u_resolution',
			'u_time',
			'u_points',
			'u_radii',
			'u_colors',
			'u_screen',
			'u_inner',
			'u_outer',
			'u_circle',
			'u_mix',
			'u_bg',
			'u_opacity'
		].map(uniform);

		return (fields, t, look, [cssWidth, cssHeight]) => {
			const scale = Math.max(CSS_PX_PER_PIXEL, Math.max(cssWidth, cssHeight) / MAX_PIXELS);
			const [width, height] = [Math.ceil(cssWidth / scale) || 1, Math.ceil(cssHeight / scale) || 1];
			if (canvas.width !== width || canvas.height !== height) {
				[canvas.width, canvas.height] = [width, height];
				gl.viewport(0, 0, width, height);
			}
			gl.uniform2f(resolution, width, height);
			gl.uniform1f(time, t);
			gl.uniform2fv(points, fields.flatMap((f) => [f.x, f.y]));
			gl.uniform1fv(radii, fields.map((f) => f.r));
			gl.uniform3fv(labs, fields.flatMap((f) => oklab(f.color)));
			gl.uniform2f(screen, cssWidth, cssHeight);
			gl.uniform2f(innerLayer, look.inner.strength, look.inner.reach);
			gl.uniform2f(outerLayer, look.outer.strength, look.outer.reach);
			gl.uniform3fv(circle, look.circle ?? [0, 0, -1]);
			gl.uniform1f(blend, look.mix);
			gl.uniform3f(bg, look.bg[0] / 255, look.bg[1] / 255, look.bg[2] / 255);
			gl.uniform1f(opacity, look.opacity);
			gl.drawArrays(gl.TRIANGLES, 0, 3);
		};
	}
</script>

<div class="pointer-events-none fixed inset-0 -z-10 overflow-hidden" aria-hidden="true">
	<canvas class="size-full scale-150" {@attach drift}></canvas>
</div>
