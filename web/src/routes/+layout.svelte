<script lang="ts">
	import '../app.css';
	import { ListMusic, LibraryBig, LoaderCircle, Settings2, WifiOff } from '@lucide/svelte';
	import { onMount, type Snippet } from 'svelte';
	import { page } from '$app/state';
	import ContextMenu from '#lib/components/ContextMenu.svelte';
	import PlayerBar from '#lib/components/PlayerBar.svelte';
	import TagEditor from '#lib/components/TagEditor.svelte';
	import Toaster from '#lib/components/Toaster.svelte';
	import { hub } from '#lib/hub.svelte.ts';
	import { library } from '#lib/library.svelte.ts';
	import '#lib/theme.svelte.ts';

	let { children }: { children: Snippet } = $props();

	const nav = [
		{ href: '/', label: 'Albums', icon: LibraryBig, active: (id: string) => id === '/' || id.startsWith('/album') },
		{ href: '/queue', label: 'Queue', icon: ListMusic, active: (id: string) => id === '/queue' },
		{ href: '/settings', label: 'Settings', icon: Settings2, active: (id: string) => id === '/settings' }
	];

	let routeId = $derived(page.route.id ?? '');
	let fullscreen = $derived(routeId === '/now');
	// Give the first connection a moment before complaining.
	let patient = $state(true);

	onMount(() => {
		hub.connect();
		const wake = () => document.visibilityState === 'visible' && hub.wake();
		document.addEventListener('visibilitychange', wake);
		const timer = setTimeout(() => (patient = false), 1500);
		return () => {
			document.removeEventListener('visibilitychange', wake);
			clearTimeout(timer);
		};
	});

	$effect(() => {
		if (hub.ready) library.sync(hub.library.revision);
	});
</script>

{#if !fullscreen}
	<aside class="fixed inset-y-0 left-0 z-20 hidden w-60 flex-col border-r border-line px-3 pt-6 pb-28 lg:flex">
		<a href="/" class="mb-7 flex items-center gap-2.5 px-3">
			<img src="/icon.svg" alt="" class="size-7 rounded-lg" />
			<span class="text-lg font-bold tracking-tight">hifi</span>
		</a>
		<nav class="flex flex-col gap-0.5">
			{#each nav as item (item.href)}
				{@const active = item.active(routeId)}
				<a
					href={item.href}
					class="flex items-center gap-3 rounded-xl px-3 py-2 text-sm font-medium transition-colors {active
						? 'bg-fg/6 text-fg'
						: 'text-muted hover:bg-fg/4 hover:text-fg'}"
					aria-current={active ? 'page' : undefined}
				>
					<item.icon class="size-[18px] {active ? 'text-accent' : ''}" />
					{item.label}
				</a>
			{/each}
		</nav>
		<div class="mt-auto px-3 text-xs text-faint">
			{#if hub.library.scanning}
				<span class="flex items-center gap-1.5"><LoaderCircle class="size-3 animate-spin" /> Scanning library…</span>
			{:else if hub.library.albums}
				{hub.library.albums.toLocaleString()} albums · {hub.library.tracks.toLocaleString()} songs
			{/if}
		</div>
	</aside>
{/if}

{#if !patient && (!hub.connected || (hub.ready && !hub.player.online))}
	<div class="pt-safe fixed inset-x-0 top-0 z-40 flex justify-center px-4">
		<div class="mt-3 flex items-center gap-2 rounded-full bg-fg px-4 py-2 text-xs font-semibold text-bg shadow-float">
			<WifiOff class="size-3.5" />
			{hub.connected ? 'MPD is not reachable' : 'Reconnecting to hifi…'}
		</div>
	</div>
{/if}

<main class={fullscreen ? '' : 'pb-[calc(9rem+env(safe-area-inset-bottom))] lg:pb-32 lg:pl-60'}>
	{@render children()}
</main>

{#if !fullscreen}
	<PlayerBar />

	<nav class="glass pb-safe fixed inset-x-0 bottom-0 z-30 border-t border-line lg:hidden">
		<div class="grid h-14 grid-cols-3">
			{#each nav as item (item.href)}
				{@const active = item.active(routeId)}
				<a
					href={item.href}
					class="flex flex-col items-center justify-center gap-0.5 text-[10px] font-semibold {active ? 'text-accent' : 'text-muted'}"
					aria-current={active ? 'page' : undefined}
				>
					<item.icon class="size-[22px]" strokeWidth={active ? 2.25 : 1.75} />
					{item.label}
				</a>
			{/each}
		</div>
	</nav>
{/if}

<TagEditor />
<ContextMenu />
<Toaster />
