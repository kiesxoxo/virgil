<script lang="ts">
	import { ChevronLeft, ListEnd, Play } from '@lucide/svelte';
	import { queue } from '../api.ts';
	import type { Group } from '../browse.ts';
	import { span } from '../format.ts';
	import { library } from '../library.svelte.ts';
	import type { Album } from '../types/Album.ts';
	import AlbumGrid from './AlbumGrid.svelte';
	import GroupArt from './GroupArt.svelte';

	// One artist's or genre's albums, with Play / Add to queue for all of them.
	let {
		kind,
		group,
		name,
		appearsOn = []
	}: { kind: 'artist' | 'genre'; group: Group | undefined; name: string; appearsOn?: Album[] } = $props();

	let albums = $derived(group?.albums ?? []);
	let meta = $derived.by(() => {
		const tracks = albums.reduce((n, a) => n + a.tracks, 0);
		const seconds = albums.reduce((n, a) => n + a.duration, 0);
		return [
			`${albums.length} ${albums.length === 1 ? 'album' : 'albums'}`,
			`${tracks.toLocaleString()} ${tracks === 1 ? 'song' : 'songs'}`,
			seconds ? span(seconds) : null
		].filter(Boolean);
	});
	let ids = $derived(albums.map((a) => a.id));
	let back = $derived(kind === 'artist' ? { href: '/artists', label: 'Artists' } : { href: '/genres', label: 'Genres' });
</script>

<svelte:head><title>{group?.name ?? name} · Virgil</title></svelte:head>

<div class="pt-safe mx-auto max-w-[110rem] px-4 lg:px-8">
	<a href={back.href} class="-ml-1.5 inline-flex h-11 items-center gap-0.5 text-[15px] font-medium text-accent lg:hidden">
		<ChevronLeft class="size-5" />
		{back.label}
	</a>

	{#if !group && library.loaded}
		<div class="mx-auto mt-16 max-w-sm text-center">
			<h1 class="text-lg font-semibold">{kind === 'artist' ? 'Artist' : 'Genre'} not found</h1>
			<p class="mt-1 text-sm text-muted">Nothing in the library goes by “{name}”.</p>
			<a href={back.href} class="mt-4 inline-block text-sm font-semibold text-accent">Back to {back.label.toLowerCase()}</a>
		</div>
	{:else}
		<header class="flex flex-col items-center gap-5 pt-2 text-center md:flex-row md:items-end md:gap-8 md:pt-10 md:text-left">
			{#if group}
				<GroupArt
					{group}
					{kind}
					size={600}
					class="w-[min(52vw,13rem)] shrink-0 shadow-cover ring-1 ring-line md:w-52 {kind === 'artist' ? 'rounded-full' : 'rounded-2xl'}"
				/>
			{:else}
				<div class="aspect-square w-[min(52vw,13rem)] shrink-0 animate-pulse bg-surface md:w-52 {kind === 'artist' ? 'rounded-full' : 'rounded-2xl'}"></div>
			{/if}
			<div class="min-w-0 flex-1">
				<p class="text-xs font-semibold tracking-wider text-muted uppercase">{kind === 'artist' ? 'Artist' : 'Genre'}</p>
				<h1 class="mt-1 text-2xl font-bold tracking-tight text-balance md:text-4xl">{group?.name ?? name}</h1>
				<p class="mt-1.5 text-sm text-muted">{meta.join(' · ')}</p>
				<div class="mt-5 flex items-center justify-center gap-2.5 md:justify-start">
					<button
						type="button"
						class="press inline-flex h-11 flex-1 items-center justify-center gap-2 rounded-full bg-accent px-7 text-[15px] font-semibold text-accent-fg shadow-float sm:flex-none"
						disabled={!ids.length}
						onclick={() => queue.playAlbums(ids)}
					>
						<Play class="size-[18px]" fill="currentColor" strokeWidth={0} /> Play
					</button>
					<button
						type="button"
						class="press inline-flex h-11 items-center gap-2 rounded-full bg-surface px-4 text-sm font-semibold hover:bg-surface-2"
						disabled={!ids.length}
						onclick={() => queue.albumsLast(ids)}
					>
						<ListEnd class="size-[18px]" /> <span class="max-sm:hidden">Add to queue</span>
					</button>
				</div>
			</div>
		</header>

		{#if albums.length}
			<section class="mt-8 md:mt-10">
				{#if appearsOn.length}<h2 class="mb-3 text-lg font-bold tracking-tight">Albums</h2>{/if}
				<AlbumGrid {albums} />
			</section>
		{/if}
		{#if appearsOn.length}
			<section class="mt-8">
				<h2 class="mb-3 text-lg font-bold tracking-tight">Appears on</h2>
				<AlbumGrid albums={appearsOn} />
			</section>
		{/if}
	{/if}
</div>
