<script lang="ts">
	import { LibraryBig, Search, X } from '@lucide/svelte';
	import AlbumGrid from '#lib/components/AlbumGrid.svelte';
	import Cover from '#lib/components/Cover.svelte';
	import Equalizer from '#lib/components/Equalizer.svelte';
	import { api, queue } from '#lib/api.ts';
	import { clock, fold } from '#lib/format.ts';
	import { hub } from '#lib/hub.svelte.ts';
	import { library } from '#lib/library.svelte.ts';
	import type { Album } from '#lib/types/Album.ts';
	import type { TrackHit } from '#lib/types/TrackHit.ts';

	type Sort = 'artist' | 'title' | 'year' | 'recent';
	const sorts: { id: Sort; label: string }[] = [
		{ id: 'artist', label: 'Artist' },
		{ id: 'title', label: 'Title' },
		{ id: 'year', label: 'Year' },
		{ id: 'recent', label: 'Recently added' }
	];

	function storedSort(): Sort {
		try {
			const s = localStorage.getItem('albumSort');
			if (sorts.some((x) => x.id === s)) return s as Sort;
		} catch {
			/* storage blocked */
		}
		return 'artist';
	}

	let query = $state('');
	let sort = $state<Sort>(storedSort());
	let songs = $state<TrackHit[]>([]);

	const collator = new Intl.Collator(undefined, { sensitivity: 'base', numeric: true });

	// Folded search text per album, computed once per library revision.
	let haystack = $derived(library.albums.map((a) => fold(`${a.title} ${a.artist} ${a.year ?? ''} ${a.genre ?? ''}`)));

	let filtered = $derived.by(() => {
		const terms = fold(query).split(/\s+/).filter(Boolean);
		let list: Album[] = terms.length
			? library.albums.filter((_, i) => terms.every((t) => haystack[i].includes(t)))
			: library.albums;
		// The server already sorts by artist, year, title.
		if (sort === 'title') list = list.toSorted((a, b) => collator.compare(a.title, b.title));
		if (sort === 'year') list = list.toSorted((a, b) => (b.year ?? 0) - (a.year ?? 0));
		if (sort === 'recent') list = list.toSorted((a, b) => b.added - a.added);
		return list;
	});

	$effect(() => {
		try {
			localStorage.setItem('albumSort', sort);
		} catch {
			/* storage blocked */
		}
	});

	// Song matches come from the server's full-text index, debounced.
	$effect(() => {
		const q = query.trim();
		if (q.length < 2) {
			songs = [];
			return;
		}
		const ctrl = new AbortController();
		const timer = setTimeout(() => {
			api.search(q, ctrl.signal).then((r) => (songs = r.tracks.slice(0, 6)), () => {});
		}, 140);
		return () => {
			clearTimeout(timer);
			ctrl.abort();
		};
	});
</script>

<svelte:head><title>Albums · hifi</title></svelte:head>

<div class="pt-safe glass sticky top-0 z-20 border-b border-line lg:border-none lg:bg-bg/80">
	<div class="mx-auto max-w-[110rem] px-4 pt-4 pb-3 lg:px-8 lg:pt-8">
		<div class="flex items-baseline justify-between gap-4">
			<h1 class="text-[28px] font-bold tracking-tight lg:text-[32px]">Albums</h1>
			{#if library.loaded}
				<span class="text-sm text-muted tabular-nums">{filtered.length.toLocaleString()}</span>
			{/if}
		</div>
		<label class="relative mt-3 block lg:max-w-md">
			<Search class="pointer-events-none absolute top-1/2 left-3.5 size-[18px] -translate-y-1/2 text-faint" />
			<input
				type="search"
				bind:value={query}
				placeholder="Albums, artists, songs"
				autocomplete="off"
				spellcheck="false"
				class="h-11 w-full rounded-xl bg-surface pr-10 pl-10 text-[15px] placeholder:text-faint focus:ring-2 focus:ring-accent/40 focus:outline-none [&::-webkit-search-cancel-button]:hidden"
			/>
			{#if query}
				<button
					type="button"
					class="absolute top-1/2 right-2 grid size-7 -translate-y-1/2 place-items-center rounded-full text-muted hover:text-fg"
					aria-label="Clear search"
					onclick={() => (query = '')}
				>
					<X class="size-4" />
				</button>
			{/if}
		</label>
		<div class="-mx-4 mt-3 flex gap-2 overflow-x-auto px-4 [scrollbar-width:none]">
			{#each sorts as s (s.id)}
				<button
					type="button"
					class="press h-8 shrink-0 rounded-full px-3.5 text-[13px] font-semibold transition-colors {sort === s.id
						? 'bg-fg text-bg'
						: 'bg-surface text-muted hover:text-fg'}"
					onclick={() => (sort = s.id)}
				>
					{s.label}
				</button>
			{/each}
		</div>
	</div>
</div>

<div class="mx-auto max-w-[110rem] px-4 pt-5 lg:px-8">
	{#if songs.length}
		<section class="mb-8">
			<h2 class="mb-2 text-sm font-semibold text-muted">Songs</h2>
			<ul class="grid gap-x-6 sm:grid-cols-2 xl:grid-cols-3">
				{#each songs as hit (hit.track.uri)}
					{@const current = hub.current?.uri === hit.track.uri}
					<li>
						<button
							type="button"
							class="press flex w-full items-center gap-3 rounded-xl p-2 text-left hover:bg-fg/4"
							onclick={() => queue.playAlbum(hit.albumId, hit.track.uri)}
						>
							<Cover art={hit.art} class="size-11 shrink-0 rounded-md" />
							<div class="min-w-0 flex-1">
								<div class="truncate text-sm font-medium {current ? 'text-accent' : ''}">{hit.track.title}</div>
								<div class="truncate text-[13px] text-muted">{hit.track.artist} · {hit.albumTitle}</div>
							</div>
							{#if current}
								<Equalizer playing={hub.playing} />
							{:else}
								<span class="text-xs text-faint tabular-nums">{clock(hit.track.duration)}</span>
							{/if}
						</button>
					</li>
				{/each}
			</ul>
		</section>
	{/if}

	{#if !library.loaded}
		<div class="grid grid-cols-2 gap-x-3.5 gap-y-6 sm:grid-cols-4 lg:grid-cols-6">
			{#each { length: 12 } as _, i (i)}
				<div class="animate-pulse">
					<div class="aspect-square rounded-xl bg-surface"></div>
					<div class="mt-3 h-3 w-3/4 rounded bg-surface"></div>
					<div class="mt-2 h-3 w-1/2 rounded bg-surface"></div>
				</div>
			{/each}
		</div>
	{:else if library.albums.length === 0}
		<div class="mx-auto mt-16 max-w-sm text-center">
			<div class="mx-auto grid size-16 place-items-center rounded-2xl bg-surface text-muted"><LibraryBig class="size-7" /></div>
			<h2 class="mt-5 text-lg font-semibold">Your library is empty</h2>
			<p class="mt-1.5 text-sm text-muted">
				Point MPD's <code class="rounded bg-surface px-1">music_directory</code> at your music, then rescan from
				<a href="/settings" class="font-medium text-accent">Settings</a>.
			</p>
		</div>
	{:else if filtered.length === 0}
		<p class="mt-12 text-center text-sm text-muted">No albums match “{query}”.</p>
	{:else}
		<AlbumGrid albums={filtered} />
	{/if}
</div>
