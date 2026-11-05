<script lang="ts">
	import { ArrowDown, ArrowUp } from '@lucide/svelte';
	import AlbumGrid from '#lib/components/AlbumGrid.svelte';
	import AlbumList, { LIST_COLUMNS_MIN_WIDTH, listColumns } from '#lib/components/AlbumList.svelte';
	import AlbumTuner from '#lib/components/AlbumTuner.svelte';
	import Cover from '#lib/components/Cover.svelte';
	import Equalizer from '#lib/components/Equalizer.svelte';
	import LibraryBar from '#lib/components/LibraryBar.svelte';
	import Masthead from '#lib/components/Masthead.svelte';
	import SortWord from '#lib/components/SortWord.svelte';
	import ViewToggle from '#lib/components/ViewToggle.svelte';
	import { trackHeader, trackItems } from '#lib/actions.ts';
	import { api, queue } from '#lib/api.ts';
	import { clock, fold, qualityRank } from '#lib/format.ts';
	import { hub } from '#lib/hub.svelte.ts';
	import { library } from '#lib/library.svelte.ts';
	import { contextMenu } from '#lib/menu.svelte.ts';
	import { search } from '#lib/search.svelte.ts';
	import { isAlbumView, isBoolean, store, stored, type AlbumView } from '#lib/stored.ts';
	import type { Album } from '#lib/types/Album.ts';
	import type { TrackHit } from '#lib/types/TrackHit.ts';

	type SortKey = 'artist' | 'title' | 'year' | 'recent' | 'genre' | 'tracks' | 'length' | 'quality';
	interface Sort {
		key: SortKey;
		desc: boolean;
	}

	const chips: { key: SortKey; label: string }[] = [
		{ key: 'artist', label: 'Artist' },
		{ key: 'title', label: 'Title' },
		{ key: 'year', label: 'Year' },
		{ key: 'recent', label: 'Added' }
	];
	const columns: { key: SortKey; label: string; numeric?: boolean }[] = [
		{ key: 'title', label: 'Title' },
		{ key: 'artist', label: 'Artist' },
		{ key: 'year', label: 'Year' },
		{ key: 'genre', label: 'Genre' },
		{ key: 'tracks', label: 'Tracks', numeric: true },
		{ key: 'length', label: 'Length', numeric: true },
		{ key: 'quality', label: 'Quality', numeric: true }
	];
	/** Direction a key starts in: names A→Z, numbers biggest/newest first. */
	const startsDesc: Record<SortKey, boolean> = {
		artist: false,
		title: false,
		genre: false,
		year: true,
		recent: true,
		tracks: true,
		length: true,
		quality: true
	};

	const isSort = (v: unknown): v is Sort =>
		typeof v === 'object' && v !== null && (v as Sort).key in startsDesc && typeof (v as Sort).desc === 'boolean';

	let sort = $state<Sort>(stored('albumSort', isSort, { key: 'artist', desc: false }));
	// A saved "flow" is Cover Flow, which the tuner replaced.
	let view = $state<AlbumView>(stored('albumView', isAlbumView, stored('albumView', (v): v is 'flow' => v === 'flow', null) ? 'tuner' : 'grid'));
	let listArt = $state(stored('albumListArt', isBoolean, true));
	/** The album in the middle of the screen as the view last changed: the next view opens on it. */
	let reveal = $state<string | null>(null);

	/** The album at the middle of the screen, or just above or below it where that falls in a gap. */
	function centredAlbum(): string | null {
		for (const dy of [0, -80, 80, -160, 160]) {
			const tile = document.elementFromPoint(innerWidth / 2, innerHeight / 2 + dy)?.closest<HTMLElement>('[data-album]');
			if (tile) return tile.dataset.album ?? null;
		}
		return null;
	}

	// The next view opens on the album in the middle of this one.
	function switchView(next: AlbumView) {
		if (next === view) return;
		reveal = centredAlbum();
		view = next;
	}
	let songs = $state<TrackHit[]>([]);
	let contentWidth = $state(0);
	let wide = $derived(contentWidth >= LIST_COLUMNS_MIN_WIDTH);

	function sortBy(key: SortKey) {
		sort = sort.key === key ? { key, desc: !sort.desc } : { key, desc: startsDesc[key] };
	}

	const collator = new Intl.Collator(undefined, { sensitivity: 'base', numeric: true });

	// Ties keep the server's artist/year/title order (toSorted is stable).
	const compare: Record<SortKey, ((a: Album, b: Album) => number) | null> = {
		artist: null,
		title: (a, b) => collator.compare(a.title, b.title),
		year: (a, b) => (a.year ?? 0) - (b.year ?? 0),
		recent: (a, b) => a.added - b.added,
		genre: (a, b) => collator.compare(a.genre ?? '', b.genre ?? ''),
		tracks: (a, b) => a.tracks - b.tracks,
		length: (a, b) => a.duration - b.duration,
		quality: (a, b) => qualityRank(a.format) - qualityRank(b.format)
	};

	// Folded search text per entry (all its versions), computed once per library revision.
	let haystack = $derived(
		library.entries.map((entry) =>
			fold(
				library
					.versions(entry.id)
					.map((a) => `${a.title} ${a.artist} ${a.year ?? ''} ${a.genre ?? ''}`)
					.join(' ')
			)
		)
	);

	let filtered = $derived.by(() => {
		const terms = fold(search.query).split(/\s+/).filter(Boolean);
		let list: Album[] = terms.length
			? library.entries.filter((_, i) => terms.every((t) => haystack[i].includes(t)))
			: library.entries;
		const cmp = compare[sort.key];
		if (cmp) list = list.toSorted(sort.desc ? (a, b) => cmp(b, a) : cmp);
		else if (sort.desc) list = list.toReversed();
		return list;
	});

	$effect(() => store({ albumSort: sort, albumView: view, albumListArt: listArt }));

	// Song matches come from the server's full-text index, debounced.
	$effect(() => {
		const q = search.query.trim();
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

<svelte:head><title>Albums · Virgil</title></svelte:head>

<div class="pt-safe">
	<Masthead word="ALBUMS" />
</div>

<LibraryBar placeholder="Albums, artists, songs" count={library.loaded ? filtered.length : null}>
	{#snippet controls()}
		<ViewToggle bind:view={() => view, switchView} bind:art={listArt} tuner />
	{/snippet}
	{#snippet order()}
		{#each chips as chip (chip.key)}
			<SortWord label={chip.label} active={sort.key === chip.key} desc={sort.desc} onclick={() => sortBy(chip.key)} />
		{/each}
	{/snippet}
	{#snippet heads()}
		{#if view === 'list' && wide && library.albums.length}
			<div class="flex h-8 items-stretch border-t border-line" role="row">
				<div class="grid min-w-0 flex-1 items-center gap-x-4 {listArt ? '' : 'pl-3'}" style:grid-template-columns={listColumns(listArt)}>
					{#if listArt}<span></span>{/if}
					{#each columns as column (column.key)}
						{@const active = sort.key === column.key}
						<button
							type="button"
							role="columnheader"
							aria-sort={active ? (sort.desc ? 'descending' : 'ascending') : 'none'}
							class="label flex items-center gap-1 text-[10.5px] {column.numeric ? 'justify-end' : ''} {active ? 'text-accent' : 'text-muted hover:text-fg'}"
							onclick={() => sortBy(column.key)}
						>
							{column.label}
							{#if active}
								{#if sort.desc}<ArrowDown class="size-3" />{:else}<ArrowUp class="size-3" />{/if}
							{/if}
						</button>
					{/each}
				</div>
				<span class="w-11 shrink-0"></span>
			</div>
		{/if}
	{/snippet}
</LibraryBar>

<div bind:clientWidth={contentWidth}>
	{#if songs.length}
		<section class="border-b border-line">
			<h2 class="label flex h-9 items-center border-b border-line px-3 text-[11px] text-muted">Songs</h2>
			<ul class="grid sm:grid-cols-2 xl:grid-cols-3">
				{#each songs as hit (hit.track.uri)}
					{@const current = hub.current?.uri === hit.track.uri}
					<li
						class="border-b border-line last:border-b-0"
						{@attach contextMenu(
							() => trackItems(hit.track, hit.albumId),
							() => trackHeader(hit.track, hit.art)
						)}
					>
						<button type="button" class="press flex w-full items-center gap-3 pr-3 text-left" onclick={() => queue.playAlbum(hit.albumId, hit.track.uri)}>
							<Cover art={hit.art} class="size-14 shrink-0" />
							<div class="min-w-0 flex-1">
								<div class="wide truncate text-[13px] leading-[18px] {current ? 'text-accent' : ''}">{hit.track.title}</div>
								<div class="truncate text-xs leading-4 text-muted">{hit.track.artist} · {hit.albumTitle}</div>
							</div>
							{#if current}
								<Equalizer playing={hub.playing} />
							{:else}
								<span class="label text-[11px] text-muted">{clock(hit.track.duration)}</span>
							{/if}
						</button>
					</li>
				{/each}
			</ul>
		</section>
	{/if}

	{#if !library.loaded}
		<div class="grid grid-cols-2 sm:grid-cols-4 lg:grid-cols-6">
			{#each { length: 12 } as _, i (i)}
				<div class="border-r border-b border-line">
					<div class="aspect-square bg-surface"></div>
					<div class="h-[72px]"></div>
				</div>
			{/each}
		</div>
	{:else if library.albums.length === 0}
		<div class="px-3 pt-8">
			<h2 class="wide text-2xl">Your library is empty</h2>
			<p class="mt-2 text-sm text-muted">
				Point MPD's <code class="font-mono">music_directory</code> at your music, then rescan from
				<a href="/settings" class="text-accent underline">Settings</a>.
			</p>
		</div>
	{:else if filtered.length === 0}
		<p class="label mt-12 px-3 text-center text-[11px] text-muted">No albums match “{search.query}”</p>
	{:else if view === 'list'}
		<AlbumList albums={filtered} art={listArt} {wide} {reveal} />
	{:else if view === 'tuner'}
		<AlbumTuner albums={filtered} sort={sort.key} {reveal} />
	{:else}
		<AlbumGrid albums={filtered} {reveal} />
	{/if}
</div>
