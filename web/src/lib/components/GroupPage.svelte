<script lang="ts">
	import { ChevronLeft, ListEnd } from '@lucide/svelte';
	import { queue } from '../api.ts';
	import type { Group } from '../browse.ts';
	import { fitTitle } from '../fit.ts';
	import { span } from '../format.ts';
	import { library } from '../library.svelte.ts';
	import type { Album } from '../types/Album.ts';
	import AlbumGrid from './AlbumGrid.svelte';
	import Glyph from './Glyph.svelte';

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
	let room = $state(0);
</script>

<svelte:head><title>{group?.name ?? name} · Virgil</title></svelte:head>

<div class="pt-safe">
	<a href={back.href} data-back class="press label flex h-11 items-center gap-1 border-b border-line pl-2 text-[11px] lg:hidden">
		<ChevronLeft class="size-4" />
		{back.label}
	</a>

	{#if !group && library.loaded}
		<div class="px-3">
			<h1 class="wide mt-16 text-2xl">{kind === 'artist' ? 'Artist' : 'Genre'} not found</h1>
			<p class="mt-2 text-sm text-muted">Nothing in the library goes by “{name}”.</p>
			<a href={back.href} class="label mt-4 inline-block text-[11px] text-accent">Back to {back.label.toLowerCase()}</a>
		</div>
	{:else}
		<header class="border-b border-line">
			<div class="px-3 pt-3 pb-3" bind:clientWidth={room}>
				<p class="label text-[10.5px] text-muted">{kind === 'artist' ? 'Artist' : 'Genre'}</p>
				<h1 class="wide mt-1.5 leading-[1.03] text-balance" style:font-size="{room ? fitTitle(group?.name ?? name, room - 24, { scale: 1.1, max: 72 }) : 32}px">{group?.name ?? name}</h1>
				<p class="label mt-2 text-[10.5px] text-muted">{meta.join(' / ')}</p>
			</div>
			<div class="grid h-14 grid-cols-[minmax(0,1fr)_56px] border-t border-line md:max-w-md md:border-r">
				<button
					type="button"
					class="press wide flex items-center justify-center gap-2.5 bg-accent text-[17px] text-accent-fg disabled:bg-surface-2 disabled:text-faint"
					disabled={!ids.length}
					onclick={() => queue.playAlbums(ids)}
				>
					<Glyph name="play" class="size-4" /> PLAY
				</button>
				<button
					type="button"
					class="press grid place-items-center border-l border-line disabled:text-faint"
					disabled={!ids.length}
					aria-label="Add to queue"
					title="Add to queue"
					onclick={() => queue.albumsLast(ids)}
				>
					<ListEnd class="size-5" />
				</button>
			</div>
		</header>

		{#if albums.length}
			<section>
				{#if appearsOn.length}<h2 class="label flex h-9 items-center border-b border-line px-3 text-[11px] text-muted">Albums</h2>{/if}
				<AlbumGrid {albums} />
			</section>
		{/if}
		{#if appearsOn.length}
			<section>
				<h2 class="label flex h-9 items-center border-b border-line px-3 text-[11px] text-muted">Appears on</h2>
				<AlbumGrid albums={appearsOn} />
			</section>
		{/if}
	{/if}
</div>
