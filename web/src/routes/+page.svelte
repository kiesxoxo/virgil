<script lang="ts">
	import { ArrowDown, ArrowUp, Image, ImageOff, LayoutGrid, LibraryBig, List, Search, X } from '@lucide/svelte';
	import AlbumGrid from '#lib/components/AlbumGrid.svelte';
	import AlbumList, { LIST_COLUMNS_MIN_WIDTH, listColumns } from '#lib/components/AlbumList.svelte';
	import Cover from '#lib/components/Cover.svelte';
	import Equalizer from '#lib/components/Equalizer.svelte';
	import { trackHeader, trackItems } from '#lib/actions.ts';
	import { api, queue } from '#lib/api.ts';
	import { clock, fold, qualityRank } from '#lib/format.ts';
	import { hub } from '#lib/hub.svelte.ts';
	import { library } from '#lib/library.svelte.ts';
	import { contextMenu } from '#lib/menu.svelte.ts';
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
		{ key: 'recent', label: 'Recently added' }
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

	function stored<T>(key: string, valid: (v: unknown) => v is T, fallback: T): T {
		try {
			const v = JSON.parse(localStorage.getItem(key) ?? 'null');
			if (valid(v)) return v;
		} catch {
			/* storage blocked or old format */
		}
		return fallback;
	}

	const isSort = (v: unknown): v is Sort =>
		typeof v === 'object' && v !== null && (v as Sort).key in startsDesc && typeof (v as Sort).desc === 'boolean';

	let query = $state('');
	let sort = $state<Sort>(stored('albumSort', isSort, { key: 'artist', desc: false }));
	let view = $state<'grid' | 'list'>(stored('albumView', (v): v is 'grid' | 'list' => v === 'grid' || v === 'list', 'grid'));
	let listArt = $state(stored('albumListArt', (v): v is boolean => typeof v === 'boolean', true));
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
		const terms = fold(query).split(/\s+/).filter(Boolean);
		let list: Album[] = terms.length
			? library.entries.filter((_, i) => terms.every((t) => haystack[i].includes(t)))
			: library.entries;
		const cmp = compare[sort.key];
		if (cmp) list = list.toSorted(sort.desc ? (a, b) => cmp(b, a) : cmp);
		else if (sort.desc) list = list.toReversed();
		return list;
	});

	$effect(() => {
		try {
			localStorage.setItem('albumSort', JSON.stringify(sort));
			localStorage.setItem('albumView', JSON.stringify(view));
			localStorage.setItem('albumListArt', JSON.stringify(listArt));
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

<svelte:head><title>Albums · Virgil</title></svelte:head>

<div class="pt-safe glass sticky top-0 z-20 border-b border-line lg:border-none lg:bg-bg/80">
	<div class="mx-auto max-w-[110rem] px-4 pt-4 pb-3 lg:px-8 lg:pt-8">
		<div class="flex items-center justify-between gap-3">
			<h1 class="text-[28px] font-bold tracking-tight lg:text-[32px]">Albums</h1>
			<div class="flex items-center gap-2">
				{#if library.loaded}
					<span class="mr-1 text-sm text-muted tabular-nums">{filtered.length.toLocaleString()}</span>
				{/if}
				{#if view === 'list'}
					<button
						type="button"
						class="press grid size-8 place-items-center rounded-lg bg-surface {listArt ? 'text-fg' : 'text-muted'} hover:text-fg"
						aria-pressed={listArt}
						aria-label={listArt ? 'Hide artwork' : 'Show artwork'}
						title={listArt ? 'Hide artwork' : 'Show artwork'}
						onclick={() => (listArt = !listArt)}
					>
						{#if listArt}<Image class="size-4" />{:else}<ImageOff class="size-4" />{/if}
					</button>
				{/if}
				<div class="flex rounded-lg bg-surface p-0.5" role="group" aria-label="Layout">
					{#each [{ id: 'grid', label: 'Grid', icon: LayoutGrid }, { id: 'list', label: 'List', icon: List }] as const as option (option.id)}
						<button
							type="button"
							class="grid size-7 place-items-center rounded-md transition-colors {view === option.id
								? 'bg-bg text-fg shadow-sm ring-1 ring-line'
								: 'text-muted hover:text-fg'}"
							aria-pressed={view === option.id}
							aria-label="{option.label} view"
							title="{option.label} view"
							onclick={() => (view = option.id)}
						>
							<option.icon class="size-4" />
						</button>
					{/each}
				</div>
			</div>
		</div>
		<label class="relative mt-3 block lg:max-w-md">
			<Search class="pointer-events-none absolute top-1/2 left-3.5 size-[18px] -translate-y-1/2 text-faint" />
			<input
				type="search"
				bind:value={query}
				placeholder="Albums, artists, songs"
				autocomplete="off"
				spellcheck="false"
				class="h-11 w-full rounded-xl bg-surface pr-10 pl-10 text-base placeholder:text-faint focus:ring-2 focus:ring-accent/40 focus:outline-none [&::-webkit-search-cancel-button]:hidden"
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
			{#each chips as chip (chip.key)}
				{@const active = sort.key === chip.key}
				<button
					type="button"
					class="press inline-flex h-8 shrink-0 items-center gap-1 rounded-full px-3.5 text-[13px] font-semibold transition-colors {active
						? 'bg-fg text-bg'
						: 'bg-surface text-muted hover:text-fg'}"
					aria-pressed={active}
					onclick={() => sortBy(chip.key)}
				>
					{chip.label}
					{#if active}
						{#if sort.desc}<ArrowDown class="size-3.5" />{:else}<ArrowUp class="size-3.5" />{/if}
					{/if}
				</button>
			{/each}
		</div>
		{#if view === 'list' && wide && library.albums.length}
			<div class="mt-3 -mb-1 flex items-center border-t border-line pt-2" role="row">
				<div class="grid min-w-0 flex-1 items-center gap-4 pl-2" style:grid-template-columns={listColumns(listArt)}>
					{#if listArt}<span></span>{/if}
					{#each columns as column (column.key)}
						{@const active = sort.key === column.key}
						<button
							type="button"
							role="columnheader"
							aria-sort={active ? (sort.desc ? 'descending' : 'ascending') : 'none'}
							class="flex items-center gap-1 text-[11px] font-semibold tracking-wider uppercase {column.numeric
								? 'justify-end'
								: ''} {active ? 'text-fg' : 'text-muted hover:text-fg'}"
							onclick={() => sortBy(column.key)}
						>
							{column.label}
							{#if active}
								{#if sort.desc}<ArrowDown class="size-3" />{:else}<ArrowUp class="size-3" />{/if}
							{/if}
						</button>
					{/each}
				</div>
				<span class="w-10 shrink-0"></span>
			</div>
		{/if}
	</div>
</div>

<div class="mx-auto max-w-[110rem] px-4 lg:px-8 {view === 'list' ? 'pt-2' : 'pt-5'}" bind:clientWidth={contentWidth}>
	{#if songs.length}
		<section class="mb-8">
			<h2 class="mb-2 text-sm font-semibold text-muted">Songs</h2>
			<ul class="grid gap-x-6 sm:grid-cols-2 xl:grid-cols-3">
				{#each songs as hit (hit.track.uri)}
					{@const current = hub.current?.uri === hit.track.uri}
					<li
						{@attach contextMenu(
							() => trackItems(hit.track, hit.albumId),
							() => trackHeader(hit.track, hit.art)
						)}
					>
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
	{:else if view === 'list'}
		<AlbumList albums={filtered} art={listArt} {wide} />
	{:else}
		<AlbumGrid albums={filtered} />
	{/if}
</div>
