<script lang="ts" module>
	/** The album in the middle, so coming back from its page (or to this view) lands on it again. */
	let centred: string | null = null;
</script>

<script lang="ts">
	import { Dice1, Dice2, Dice3, Dice4, Dice5, Dice6, Play } from '@lucide/svelte';
	import { untrack } from 'svelte';
	import { goto } from '$app/navigation';
	import { albumHeader, albumItems } from '../actions.ts';
	import { colorsOf } from '../ambient.svelte.ts';
	import { queue } from '../api.ts';
	import { TOP, hitTest, layout, spin, turnAngles, type Turn } from '../coverflow.ts';
	import { flight } from '../flight.svelte.ts';
	import { TILE, artUrl } from '../format.ts';
	import { glitter } from '../glitter.ts';
	import { contextMenu } from '../menu.svelte.ts';
	import { motion } from '../motion.svelte.ts';
	import type { Rgb } from '../palette.ts';
	import { flowCards, readTheme, stage, type Scene } from '../stage-gl.ts';
	import { theme } from '../theme.svelte.ts';
	import type { Album } from '../types/Album.ts';
	import Cover from './Cover.svelte';

	/** `reveal`: the album to open on (in the middle of the screen in the view before). */
	let { albums, reveal = null }: { albums: Album[]; reveal?: string | null } = $props();

	/** Snap points kept in the DOM on each side of the middle cover. */
	const REACH = 10;

	const still = matchMedia('(prefers-reduced-motion: reduce)');
	// The covers are drawn with WebGL (lib/stage-gl.ts), straight from the scroll position:
	// scrolling changes nothing here but the cover in the middle, once per cover.
	const renderer = stage();
	untrack(() => reveal && (centred = reveal));

	let scroller = $state() as HTMLDivElement;
	let link = $state<HTMLAnchorElement>();
	let width = $state(0);
	let innerHeight = $state(800);
	/** The page's height that isn't for the covers: what is above them, and the bars below. */
	let chrome = $state(0);
	let current = $state(0);
	/** At rest on the middle cover, not between two. */
	let settled = $state(true);
	/** The phone's tilt. Not state: it changes every frame while the phone moves, and only the canvas cares. */
	let turn: Turn = [0, 0];

	/** The dice rolling: the flow spinning to an album picked at random. */
	let rolling = $state(false);
	/** Rolls landed: each one's pick gets its moment. */
	let rolls = $state(0);
	/** The die's face up, and how far it has turned (degrees). */
	let face = $state(5);
	let tumble = $state(0);
	const faces = [Dice1, Dice2, Dice3, Dice4, Dice5, Dice6];
	/** While it spins, where the flow is drawn (in covers): finer than the scroll position, which may be whole pixels. */
	let spinning: number | null = null;
	/** Where it is spinning to: the covers there are fetched first. */
	let heading: number | null = null;
	/** How far the pick comes out towards the viewer as it lands, px. */
	let lift = 0;
	let rollFrame = 0;
	/** Picked lately: not again for a while. */
	const picked: string[] = [];
	let play = $state<HTMLButtonElement>();
	let dice = $state<HTMLButtonElement>();

	let l = $derived(layout(width, innerHeight - chrome));
	let snaps = $derived.by(() => {
		const first = Math.max(0, current - REACH);
		return Array.from({ length: Math.min(albums.length, current + REACH + 1) - first }, (_, i) => first + i);
	});
	let album = $derived(albums[current]);
	let slot = $derived(album ? `album:${album.id}` : '');
	/** The cover a page transition morphs, where it can't fly: a real image over the canvas. */
	let moving = $derived(settled && !!album && flight.morphing === slot);

	const position = () => scroller.scrollLeft / l.slot;

	// Untracked: drawing at once from an effect must not make the effect depend on the scroll.
	const scene = (): Scene =>
		untrack(() => {
			// Read now, not in the callbacks: the stage may still draw this flow, fading out, after it left the page.
			const [list, middle, morphing] = [albums, current, moving];
			return {
				layout: l,
				position: spinning ?? (scroller ? position() : current),
				count: list.length,
				art: (index) => list[index]?.art ?? null,
				// Covers in the air, and the middle one shown by the page for a page transition to morph.
				// Untracked too: called while drawing, which the flow does at once from its attachment and
				// effects; a cover landing would run them again (the flow fading in anew, its scroll reset).
				drawn: (index) =>
					untrack(() => (flight.airborne(`album:${list[index]?.id}`) ? 'none' : index === middle && morphing ? 'reflection' : 'all')),
				settled,
				turn,
				lift,
				heading: heading ?? undefined
			};
		});

	// The same album stays in the middle when the list changes (search, sort) or the size does.
	$effect(() => {
		if (!width) return;
		untrack(stopRoll);
		const index = Math.max(0, albums.findIndex((a) => a.id === untrack(() => centred)));
		// Snapping off meanwhile: Chrome would re-snap to the cover it snapped to before the jump.
		scroller.style.scrollSnapType = 'none';
		scroller.scrollLeft = index * l.slot;
		current = index;
		settled = true;
		// Now, not on the next frame: a page transition may be taking its picture of this one.
		renderer.draw();
		const frame = requestAnimationFrame(() => (scroller.style.scrollSnapType = ''));
		return () => cancelAnimationFrame(frame);
	});

	$effect(() => {
		void [l, albums, moving, settled, flight.key, flight.between];
		renderer.invalidate();
	});

	$effect(() => {
		void theme.dark;
		renderer.setTheme(readTheme());
	});

	function onscroll() {
		const left = scroller.scrollLeft;
		const index = Math.min(albums.length - 1, Math.max(0, Math.round(left / l.slot)));
		current = index;
		// Passing a cover on its way to the pick isn't resting on it.
		settled = !rolling && Math.abs(left - index * l.slot) < 1;
		centred = albums[index]?.id ?? centred;
		renderer.invalidate();
	}

	$effect(() => () => {
		cancelAnimationFrame(rollFrame);
		glitter.drop();
	});

	/**
	 * Rolls the dice: picks an album at random (not one picked lately) and spins the flow to it,
	 * the die tumbling as the covers go by. Landing, the pick jumps out at you and glitter bursts
	 * out of it.
	 */
	function roll() {
		if (rolling || albums.length < 2) return;
		const count = albums.length;
		let to = current;
		for (let tries = 0; tries < 20 && (to === current || picked.includes(albums[to].id)); tries++) to = Math.floor(Math.random() * count);
		if (to === current) to = (current + 1 + Math.floor(Math.random() * (count - 1))) % count;
		picked.push(albums[to].id);
		if (picked.length > Math.min(12, count / 2)) picked.shift();
		// Its colours for the glitter, on their way already.
		const art = artUrl(albums[to].art, TILE);
		const colors = art ? colorsOf(art) : Promise.resolve(null);

		rolling = true;
		settled = false;
		heading = to;
		scroller.style.scrollSnapType = 'none';
		const course = spin(position(), to, count);
		const start = performance.now();
		let [shown, flipped] = [current, 0];
		const frame = (now: number) => {
			const t = (now - start) / 1000;
			const done = still.matches || t >= course.duration;
			spinning = done ? to : course.at(t);
			scroller.scrollLeft = spinning * l.slot;
			renderer.invalidate();
			// The die turns over as covers pass, slower as they do.
			const index = Math.round(spinning);
			if (index !== shown && now - flipped > 70) {
				[shown, flipped] = [index, now];
				face = 1 + ((face + Math.floor(Math.random() * 5)) % 6);
				tumble += Math.random() < 0.5 ? 90 : -90;
			}
			if (done) land(to, colors);
			else rollFrame = requestAnimationFrame(frame);
		};
		rollFrame = requestAnimationFrame(frame);
	}

	function land(to: number, colors: Promise<Rgb[] | null>) {
		spinning = heading = null;
		rolling = false;
		scroller.scrollLeft = to * l.slot;
		onscroll();
		requestAnimationFrame(() => (scroller.style.scrollSnapType = ''));
		rolls++;
		face = 1 + Math.floor(Math.random() * 6);
		tumble += 360 + (Math.random() < 0.5 ? 90 : 0);

		// Where the pick is on the screen, at rest.
		const r = scroller.getBoundingClientRect();
		const from = new DOMRect(r.left + l.width / 2 - l.cover / 2, r.top + TOP, l.cover, l.cover);
		const ledges = [play, dice, ...document.querySelectorAll('[data-bar]')].filter((el) => !!el);
		// Not waiting long for its colours: the glitter comes as it lands.
		const late = new Promise<null>((resolve) => setTimeout(() => resolve(null), 150));
		void Promise.race([colors, late]).then((cover) => glitter.burst(from, { cover, ledges, dark: theme.dark }));
		if (still.matches) return;
		// Out at the viewer and back, settling with a little bounce.
		const start = performance.now();
		const pop = (now: number) => {
			const t = (now - start) / 1000;
			lift = t < 1 ? l.cover * 0.2 * Math.exp(-t / 0.16) * Math.sin((t * Math.PI * 2) / 0.5) : 0;
			renderer.invalidate();
			rollFrame = t < 1 ? requestAnimationFrame(pop) : 0;
		};
		rollFrame = requestAnimationFrame(pop);
	}

	/** Taking the flow in hand stops the dice: it snaps to the nearest cover from where it is. */
	function stopRoll() {
		cancelAnimationFrame(rollFrame);
		rollFrame = 0;
		lift = 0;
		renderer.invalidate();
		if (!rolling) return;
		spinning = heading = null;
		rolling = false;
		scroller.style.scrollSnapType = '';
		onscroll();
	}

	/** Where a smooth scroll is heading, so steps add up before it gets there. */
	let target = 0;

	function goTo(index: number) {
		target = Math.max(0, Math.min(albums.length - 1, index));
		scroller.scrollTo({ left: target * l.slot, behavior: still.matches ? 'instant' : 'smooth' });
	}

	function step(by: number) {
		goTo((Math.abs(target - position()) < 1.5 ? target : current) + by);
	}

	function onkeydown(e: KeyboardEvent) {
		const to = { ArrowLeft: -1, ArrowRight: 1, Home: -albums.length, End: albums.length }[e.key];
		if (to === undefined) return;
		e.preventDefault();
		stopRoll();
		step(to);
		link?.focus({ preventScroll: true });
	}

	/** The cover under the pointer, or -1. */
	function hit(e: MouseEvent): number {
		const r = scroller.getBoundingClientRect();
		return hitTest(l, position(), albums.length, e.clientX - r.left, e.clientY - r.top, turn);
	}

	// The middle cover is a link of its own; a side cover comes to the middle.
	function onclick(e: MouseEvent) {
		if (link?.contains(e.target as Node)) return;
		const index = hit(e);
		if (index === current) goto(`/album/${albums[index].id}`);
		else if (index >= 0) goTo(index);
	}

	/** The album a long press or right-click is on: the one under it, or the middle one. */
	let pressed = -1;
	const menuAlbum = () => albums[pressed] ?? album;

	// Arrow keys come from the middle cover's link. A mouse wheel scrolls up and down: step
	// through the covers instead. (Sideways is the scroller's.) Clicks and taps go to the cover
	// under them.
	function controls(node: HTMLElement) {
		let sum = 0;
		const onwheel = (e: WheelEvent) => {
			stopRoll();
			if (Math.abs(e.deltaY) <= Math.abs(e.deltaX)) return;
			e.preventDefault();
			sum += e.deltaY;
			if (Math.abs(sum) < 40) return;
			step(Math.sign(sum));
			sum = 0;
		};
		// Captured, so it is known before the context menu asks.
		const onpress = (e: MouseEvent) => {
			stopRoll();
			pressed = hit(e);
		};
		node.addEventListener('keydown', onkeydown);
		node.addEventListener('click', onclick);
		node.addEventListener('wheel', onwheel, { passive: false });
		node.addEventListener('pointerdown', onpress, true);
		node.addEventListener('contextmenu', onpress, true);
		return () => {
			node.removeEventListener('keydown', onkeydown);
			node.removeEventListener('click', onclick);
			node.removeEventListener('wheel', onwheel);
			node.removeEventListener('pointerdown', onpress, true);
			node.removeEventListener('contextmenu', onpress, true);
		};
	}

	// bind:clientWidth reports a frame late; the first frame (and a view transition's snapshot of
	// it) needs the real width, or the covers start at their smallest.
	function measure(node: HTMLElement) {
		const update = () => (width = node.clientWidth);
		update();
		const observer = new ResizeObserver(update);
		observer.observe(node);
		return () => observer.disconnect();
	}

	// What is above the flow and the bars below, from the top of the whole view: the covers move
	// down within it to the middle, which mustn't change how much room they have.
	function room(node: HTMLElement) {
		const update = () => {
			// The main element's bottom padding keeps the page clear of the player bar (and tab bar).
			const bars = parseFloat(getComputedStyle(node.closest('main') ?? node).paddingBottom) || 0;
			chrome = node.getBoundingClientRect().top + scrollY + bars;
		};
		update();
		const observer = new ResizeObserver(update);
		observer.observe(document.documentElement);
		return () => observer.disconnect();
	}

	// The renderer's canvas lies under the scroller, which takes the touches. The flow turns as the
	// phone moves; the middle cover's link turns with it (through --turn-x and --turn-y), so the
	// cover a page transition moves starts where the canvas drew it. Its covers in sight can fly
	// to (and from) another view.
	function canvas(node: HTMLElement) {
		renderer.show(scene, node);
		const inSight = () =>
			flowCards(scene(), node.getBoundingClientRect()).flatMap(({ index, card }) => {
				const album = albums[index];
				return album && card.art ? [{ key: `album:${album.id}`, art: card.art, place: { ...card, flow: true } }] : [];
			});
		flight.flow = inSight;
		const unsubscribe = motion.subscribe((x, y) => {
			turn = [x, y];
			const [ax, ay] = turnAngles(turn);
			node.style.setProperty('--turn-x', `${ax}rad`);
			node.style.setProperty('--turn-y', `${ay}rad`);
			renderer.invalidate();
		});
		return () => {
			unsubscribe();
			turn = [0, 0];
			if (flight.flow === inSight) flight.flow = null;
			renderer.hide();
		};
	}
</script>

<svelte:window bind:innerHeight />

<!-- The covers and their caption in the middle of the room between the page's top and the bars. -->
<div class="flex flex-col justify-center" style:min-height="{innerHeight - chrome}px" {@attach room}>
	<div class="relative -mx-4 lg:-mx-8" {@attach canvas}>
		<div
			bind:this={scroller}
			{onscroll}
			{@attach measure}
			{@attach controls}
			{@attach contextMenu(
				() => albumItems(menuAlbum()),
				() => albumHeader(menuAlbum())
			)}
			class="relative snap-x snap-mandatory overflow-x-auto overflow-y-hidden [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
			role="region"
			aria-roledescription="carousel"
			aria-label="Albums"
		>
			<div class="relative" style:width="{width + (albums.length - 1) * l.slot}px" style:height="{l.height}px">
				{#each snaps as index (index)}
					<div class="absolute top-0 h-full snap-center" style:left="{width / 2 - l.slot / 2 + index * l.slot}px" style:width="{l.slot}px"></div>
				{/each}
				{#if album}
					<!-- Where the middle cover is drawn when at rest. At rest it also holds the cover as an
					     image, shown when a page transition moves it (the canvas leaves that one out). -->
					<a
						bind:this={link}
						href="/album/{album.id}"
						data-reflection={l.reflection}
						class="absolute block rounded-xl"
						style:left="{width / 2 - l.cover / 2 + current * l.slot}px"
						style:top="{TOP}px"
						style:width="{l.cover}px"
						style:height="{l.cover}px"
						style:transform="perspective({l.perspective}px) rotateX(var(--turn-x, 0rad)) rotateY(var(--turn-y, 0rad))"
						aria-label="{album.title} by {album.artist}"
					>
						{#if settled}
							<Cover
								art={album.art}
								size={1000}
								eager
								slot="album:{album.id}"
								class="rounded-xl shadow-cover ring-1 ring-line {moving ? '' : 'opacity-0'}"
							/>
						{/if}
					</a>
				{/if}
			</div>
		</div>
	</div>

	{#if album}
		<!-- Not read out while the dice roll: the names fly by. -->
		<div class="mt-1 flex flex-col items-center px-4 text-center" aria-live={rolling ? 'off' : 'polite'}>
			{#key rolls}
				<a href="/album/{album.id}" class="max-w-full min-w-0 transition-opacity duration-300" class:opacity-50={rolling} class:picked={rolls > 0}>
					<div class="truncate text-base font-semibold">{album.title}</div>
					<div class="truncate text-sm text-muted">{album.artist}{album.year ? ` · ${album.year}` : ''}</div>
				</a>
			{/key}
			<div class="mt-3 flex items-center gap-3">
				<button
					bind:this={play}
					type="button"
					class="press relative inline-flex h-10 items-center gap-1.5 overflow-hidden rounded-full bg-accent pr-5 pl-4 text-sm font-semibold text-accent-fg shadow-float transition-opacity disabled:opacity-60"
					disabled={rolling}
					onclick={() => queue.playAlbum(album.id)}
				>
					<Play class="size-4" fill="currentColor" /> Play
					<!-- A gleam across it for the dice's pick. -->
					{#if rolls && !rolling}
						{#key rolls}<span class="shine" aria-hidden="true"></span>{/key}
					{/if}
				</button>
				{#if albums.length > 1}
					{@const Face = faces[face - 1]}
					<button
						bind:this={dice}
						type="button"
						class="dice press grid size-10 place-items-center rounded-full shadow-float"
						class:rolling
						aria-label={rolls ? 'Roll again' : 'Pick an album at random'}
						title={rolls ? 'Roll again' : 'Pick an album at random'}
						onclick={roll}
					>
						<span class="die" class:landed={!rolling} style:transform="rotate({tumble}deg)"><Face class="size-5" /></span>
					</button>
				{/if}
			</div>
		</div>
	{/if}
</div>

<style>
	/* The dice: a rainbow ring, turning while it rolls. */
	.dice {
		border: 1.5px solid transparent;
		background:
			linear-gradient(var(--surface), var(--surface)) padding-box,
			conic-gradient(from var(--ring-turn), #ff6b4f, #ffc94f, #5fe3a1, #5fb8ff, #b77bff, #ff6b4f) border-box;
	}
	.dice.rolling {
		animation: ring 700ms linear infinite;
	}
	@keyframes ring {
		to {
			--ring-turn: 360deg;
		}
	}
	.die {
		display: grid;
		transition: transform 140ms cubic-bezier(0.2, 0, 0, 1);
	}
	/* The last throw settles with a bounce. */
	.die.landed {
		transition: transform 650ms cubic-bezier(0.34, 1.56, 0.64, 1);
	}
	.shine {
		position: absolute;
		inset: 0;
		pointer-events: none;
	}
	.shine::after {
		content: '';
		position: absolute;
		top: -20%;
		bottom: -20%;
		left: -60%;
		width: 45%;
		background: linear-gradient(100deg, transparent, rgb(255 255 255 / 0.55), transparent);
		transform: skewX(-18deg);
		animation: shine 1100ms 350ms cubic-bezier(0.4, 0, 0.2, 1) both;
	}
	@keyframes shine {
		to {
			transform: translateX(420%) skewX(-18deg);
		}
	}
	.picked {
		animation: picked 550ms cubic-bezier(0.34, 1.56, 0.64, 1);
	}
	@keyframes picked {
		from {
			transform: scale(0.9);
			opacity: 0.5;
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.dice.rolling,
		.shine::after,
		.picked {
			animation: none;
		}
		.die,
		.die.landed {
			transition: none;
		}
	}
</style>
