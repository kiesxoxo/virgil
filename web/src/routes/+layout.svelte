<script lang="ts">
	import '../app.css';
	import { WifiOff } from '@lucide/svelte';
	import { onMount, type Snippet } from 'svelte';
	import { afterNavigate } from '$app/navigation';
	import { page } from '$app/state';
	import ArtistSheet from '#lib/components/ArtistSheet.svelte';
	import CatalogSheet from '#lib/components/CatalogSheet.svelte';
	import ContextMenu from '#lib/components/ContextMenu.svelte';
	import DedicationSheet from '#lib/components/DedicationSheet.svelte';
	import JoinSheet from '#lib/components/JoinSheet.svelte';
	import LineSheet from '#lib/components/LineSheet.svelte';
	import Logotype from '#lib/components/Logotype.svelte';
	import PlayerBar from '#lib/components/PlayerBar.svelte';
	import ReachRail from '#lib/components/ReachRail.svelte';
	import Shortcuts from '#lib/components/Shortcuts.svelte';
	import ThumbArc from '#lib/components/ThumbArc.svelte';
	import Toaster from '#lib/components/Toaster.svelte';
	import { trackOpeners } from '#lib/back.ts';
	import { hub } from '#lib/hub.svelte.ts';
	import { keyboard } from '#lib/keyboard.svelte.ts';
	import { library } from '#lib/library.svelte.ts';
	import { line } from '#lib/party.svelte.ts';
	import { reach } from '#lib/reach.svelte.ts';
	import { swipeBack } from '#lib/swipeback.ts';
	import { isStream, radio } from '#lib/radio.svelte.ts';
	import '#lib/theme.svelte.ts';

	let { children }: { children: Snippet } = $props();

	trackOpeners();
	// Who this device is, and its moments on the request line.
	line.watch();

	const shelves = [
		{ href: '/', label: 'Albums', active: (id: string) => id === '/' || id.startsWith('/album/') },
		{ href: '/artists', label: 'Artists', active: (id: string) => id === '/artists' || id.startsWith('/artist/') },
		{ href: '/genres', label: 'Genres', active: (id: string) => id === '/genres' || id.startsWith('/genre/') }
	];
	const pages = [
		{ href: '/radio', label: 'Radio', active: (id: string) => id === '/radio' },
		{ href: '/queue', label: 'Queue', active: (id: string) => id === '/queue' },
		{ href: '/settings', label: 'Settings', active: (id: string) => id === '/settings' }
	];
	// Desktop: every view in the sidebar.
	const nav = [...shelves, ...pages];

	let routeId = $derived(page.route.id ?? '');
	afterNavigate(() => (reach.panel = false));
	// Now Playing and the big screen take the whole screen.
	let fullscreen = $derived(routeId === '/now' || routeId === '/screen');

	/** Which of the library's pages was seen last. */
	let shelf = $state('/');
	$effect(() => {
		shelf = shelves.find((item) => item.active(routeId))?.href ?? shelf;
	});
	// Phones: one Library tab, which goes back to that page; the page switches between albums, artists and genres.
	let tabs = $derived([
		{ href: shelf, label: 'Library', active: (id: string) => shelves.some((item) => item.active(id)) },
		...pages
	]);
	// Give the first connection a moment before complaining.
	let patient = $state(true);

	onMount(() => {
		hub.connect();
		const wake = () => document.visibilityState === 'visible' && hub.wake();
		document.addEventListener('visibilitychange', wake);
		const timer = setTimeout(() => (patient = false), 1500);
		const unwatch = keyboard.watch();
		const unswipe = swipeBack();
		// Measuring on a phone: ?fps shows dropped frames (lib/fps.ts).
		let stopProbe: (() => void) | undefined;
		if (new URLSearchParams(location.search).has('fps')) import('#lib/fps.ts').then((m) => (stopProbe = m.probe()));
		return () => {
			stopProbe?.();
			document.removeEventListener('visibilitychange', wake);
			clearTimeout(timer);
			unwatch();
			unswipe();
		};
	});

	$effect(() => {
		if (hub.ready) library.sync(hub.library.revision);
	});

	// A station in the queue is shown as what's on it, so fetch the stations then.
	$effect(() => {
		if (hub.queue.items.some((item) => isStream(item.uri))) radio.load();
	});
</script>

{#if !fullscreen}
	<aside class="pt-safe fixed inset-y-0 left-0 z-20 hidden w-60 flex-col border-r border-line pb-[72px] lg:flex">
		<a href="/" class="block border-b border-line p-3"><Logotype class="w-full" /></a>
		<nav class="flex flex-col">
			{#each nav as item (item.href)}
				{@const active = item.active(routeId)}
				<a
					href={item.href}
					class="wide flex h-11 items-center border-b border-line px-3 text-[13px] uppercase {active ? 'bg-fg text-bg' : 'press text-muted hover:text-fg'}"
					aria-current={active ? 'page' : undefined}
				>
					{item.label}
				</a>
			{/each}
		</nav>
		<div class="label mt-auto p-3 text-[10px] text-muted">
			{#if hub.library.scanning}
				Scanning library…
			{:else if hub.library.albums}
				{hub.library.albums.toLocaleString()} albums<br />{hub.library.tracks.toLocaleString()} songs
			{/if}
		</div>
	</aside>
{/if}

<!-- Nothing shows through under the status bar. -->
<div class="pt-safe fixed inset-x-0 top-0 z-40 bg-bg">
	{#if !patient && (!hub.connected || (hub.ready && !hub.player.online))}
		<div class="label flex h-8 items-center justify-center gap-2 border-b border-warn text-[11px] text-warn">
			<WifiOff class="size-3.5" />
			{hub.connected ? 'MPD is not reachable' : 'Reconnecting to Virgil…'}
		</div>
	{/if}
</div>

<main class={fullscreen ? '' : 'pr-[var(--side-r)] pb-[calc(var(--nav-h)+4rem+var(--reach-row)+env(safe-area-inset-bottom))] pl-[var(--side-l)] lg:pb-[72px] lg:pl-60'}>
	{@render children()}
</main>

{#if !fullscreen}
	<PlayerBar />

	<!-- With no tab bar, the safe area under the player still has to hide the page. -->
	<div class="fixed inset-x-0 bottom-0 z-30 hidden h-[env(safe-area-inset-bottom)] bg-bg rail:block arc:block"></div>

	{#if reach.active === 'rail'}
		<ReachRail items={nav} {routeId} />
	{:else if reach.active === 'arc'}
		<ThumbArc items={nav} {routeId} />
	{/if}

	<!-- Stays beneath the on-screen keyboard rather than riding up with it. -->
	<nav class="pb-safe fixed inset-x-0 bottom-0 z-30 border-t border-line bg-bg lg:hidden rail:hidden arc:hidden" class:hidden={keyboard.height > 0}>
		<div class="grid h-14 {reach.active === 'deck' ? 'grid-cols-6' : 'grid-cols-4'}">
			{#each reach.active === 'deck' ? nav : tabs as item (item.label)}
				{@const active = item.active(routeId)}
				<a
					href={item.href}
					class="wide grid place-items-center border-l border-line text-[clamp(9px,2.8vw,11px)] uppercase first:border-l-0 deck:font-mono deck:text-[10px] deck:font-medium deck:tracking-[0.02em] {active
						? 'bg-fg text-bg'
						: 'press text-muted'}"
					aria-current={active ? 'page' : undefined}
				>
					{item.label}
				</a>
			{/each}
		</div>
	</nav>
{/if}

<Shortcuts />
<ContextMenu />
<LineSheet />
<JoinSheet />
<DedicationSheet />
<ArtistSheet />
<CatalogSheet />
<Toaster />
