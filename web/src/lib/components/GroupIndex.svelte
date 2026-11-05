<script lang="ts">
	import { ArrowDown, ArrowUp, ListEnd, Play } from '@lucide/svelte';
	import { queue } from '../api.ts';
	import { NO_GENRE, type Group } from '../browse.ts';
	import { TILE, fold, span } from '../format.ts';
	import { library } from '../library.svelte.ts';
	import { contextMenu, type MenuItem } from '../menu.svelte.ts';
	import { search } from '../search.svelte.ts';
	import { isBoolean, isView, store, stored, type View } from '../stored.ts';
	import { Rows } from '../virtual.svelte.ts';
	import GroupArt from './GroupArt.svelte';
	import LibraryBar from './LibraryBar.svelte';
	import Masthead from './Masthead.svelte';
	import Menu from './Menu.svelte';
	import SortWord from './SortWord.svelte';
	import ViewToggle from './ViewToggle.svelte';

	// The Artists and Genres pages: search, sort, and a grid of cards or a list.
	let { kind, groups, title }: { kind: 'artist' | 'genre'; groups: Group[]; title: string } = $props();

	type SortKey = 'name' | 'albums' | 'tracks' | 'length';
	const sortKeys: SortKey[] = ['name', 'albums', 'tracks', 'length'];
	const columns: { key: SortKey; label: string; numeric?: boolean }[] = [
		{ key: 'name', label: 'Name' },
		{ key: 'albums', label: 'Albums', numeric: true },
		{ key: 'tracks', label: 'Songs', numeric: true },
		{ key: 'length', label: 'Length', numeric: true }
	];
	/** Width at which the list switches from stacked rows to columns. */
	const COLUMNS_MIN_WIDTH = 560;
	const listColumns = (art: boolean) => `${art ? '3.5rem ' : ''}minmax(0,1fr) 4.5rem 4.5rem 6.5rem`;

	// The component is made afresh for each page, so reading `kind` once is fine.
	// svelte-ignore state_referenced_locally
	const prefix = kind;
	let sort = $state(
		stored<{ key: SortKey; desc: boolean }>(
			`${prefix}Sort`,
			(v): v is { key: SortKey; desc: boolean } =>
				sortKeys.includes((v as { key: SortKey })?.key) && typeof (v as { desc: boolean }).desc === 'boolean',
			{ key: 'name', desc: false }
		)
	);
	let view = $state<View>(stored(`${prefix}View`, isView, 'grid'));
	let listArt = $state(stored(`${prefix}ListArt`, isBoolean, true));
	let contentWidth = $state(0);
	let wide = $derived(contentWidth >= COLUMNS_MIN_WIDTH);

	$effect(() => store({ [`${prefix}Sort`]: sort, [`${prefix}View`]: view, [`${prefix}ListArt`]: listArt }));

	function sortBy(key: SortKey) {
		sort = sort.key === key ? { key, desc: !sort.desc } : { key, desc: key !== 'name' };
	}

	// Songs and running time per group.
	let totals = $derived(
		new Map(
			groups.map((g) => [
				g.key,
				{ tracks: g.albums.reduce((n, a) => n + a.tracks, 0), length: g.albums.reduce((n, a) => n + a.duration, 0) }
			])
		)
	);
	const stat = (g: Group, key: 'tracks' | 'length') => totals.get(g.key)?.[key] ?? 0;

	let shown = $derived.by(() => {
		const terms = fold(search.query).split(/\s+/).filter(Boolean);
		let list = terms.length ? groups.filter((g) => terms.every((t) => g.key.includes(t))) : groups;
		// Groups arrive A–Z; "No genre" stays last whatever the order.
		const last = list.filter((g) => g.key === NO_GENRE);
		list = list.filter((g) => g.key !== NO_GENRE);
		const key = sort.key;
		if (key === 'albums') list = list.toSorted((a, b) => a.albums.length - b.albums.length);
		else if (key !== 'name') list = list.toSorted((a, b) => stat(a, key) - stat(b, key));
		if (sort.desc) list = list.toReversed();
		return [...list, ...last];
	});

	// Window-scrolled and virtual, as the albums are: only rows near the viewport exist in the DOM.
	// The grid's columns as Tailwind's breakpoints had them, butted up and divided by rules.
	/** The name (two lines at most), the count, and the rule under them. */
	let META = $derived(kind === 'artist' ? 66 : 72);
	const rows: Rows = new Rows(() => ({ count: shown.length, columns: perRow, rowHeight, overscan: view === 'list' ? 4 : 3 }));
	let screenWidth = $state(typeof window === 'undefined' ? 1024 : innerWidth);
	let cols = $derived.by(() => {
		const [xl, lg, sm, xs] = kind === 'artist' ? [8, 6, 4, 3] : [6, 5, 3, 2];
		return screenWidth >= 1536 ? xl : screenWidth >= 1024 ? lg : screenWidth >= 640 ? sm : xs;
	});
	// Whole pixels, so that no cover lands between two.
	let tile = $derived(Math.max(0, Math.floor(rows.width / cols)));
	let rowHeight = $derived(view === 'list' ? (listArt ? 56 : wide ? 44 : 52) : tile + META);
	let perRow = $derived(view === 'list' ? 1 : cols);
	let visible = $derived(rows.ready ? shown.slice(rows.first * perRow, rows.last * perRow) : []);

	const href = (g: Group) => `/${kind}/${encodeURIComponent(g.name)}`;
	const ids = (g: Group) => g.albums.map((a) => a.id);
	const items = (g: Group): MenuItem[] => [
		{ label: 'Play', icon: Play, run: () => queue.playAlbums(ids(g)) },
		{ label: 'Add to end of queue', icon: ListEnd, run: () => queue.albumsLast(ids(g)) }
	];
	const count = (n: number, word: string) => `${n.toLocaleString()} ${word}${n === 1 ? '' : 's'}`;
	const header = (g: Group) => ({ title: g.name, subtitle: count(g.albums.length, 'album') });
</script>

<svelte:head><title>{title} · Virgil</title></svelte:head>
<svelte:window bind:innerWidth={screenWidth} />

<div class="pt-safe">
	<Masthead word={kind === 'artist' ? 'ARTISTS' : 'GENRES'} />
</div>

<LibraryBar placeholder="Search {title.toLowerCase()}" count={library.loaded ? shown.length : null}>
	{#snippet controls()}
		<ViewToggle bind:view bind:art={listArt} />
	{/snippet}
	{#snippet order()}
		{#each [{ key: 'name', label: 'Name' }, { key: 'albums', label: 'Most albums' }] as const as chip (chip.key)}
			<SortWord label={chip.label} active={sort.key === chip.key} desc={sort.desc} onclick={() => sortBy(chip.key)} />
		{/each}
	{/snippet}
	{#snippet heads()}
		{#if view === 'list' && wide && shown.length}
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
	{#if !library.loaded}
		<div class="grid {kind === 'artist' ? 'grid-cols-3 sm:grid-cols-4 lg:grid-cols-6 2xl:grid-cols-8' : 'grid-cols-2 sm:grid-cols-3 lg:grid-cols-5 2xl:grid-cols-6'}">
			{#each { length: 12 } as _, i (i)}
				<div class="border-r border-b border-line">
					<div class="aspect-square bg-surface"></div>
					<div class="h-[66px]"></div>
				</div>
			{/each}
		</div>
	{:else if shown.length === 0}
		<p class="label mt-12 px-3 text-center text-[11px] text-muted">
			{search.query ? `No ${title.toLowerCase()} match “${search.query}”` : `No ${title.toLowerCase()} yet`}
		</p>
	{:else if view === 'list'}
		<div role="table" aria-label={title} aria-rowcount={shown.length} class="relative" style:height="{shown.length * rowHeight}px" {@attach rows.attach}>
			{#each visible as group, i (group.key)}
				{@const none = group.key === NO_GENRE}
				<div
					role="row"
					aria-rowindex={rows.first + i + 1}
					class="absolute inset-x-0 top-0 flex items-stretch border-b border-line hover:bg-surface"
					style:height="{rowHeight}px"
					style:transform="translateY({(rows.first + i) * rowHeight}px)"
					{@attach contextMenu(() => items(group), () => header(group))}
				>
					<a
						href={href(group)}
						class="min-w-0 flex-1 {listArt ? '' : 'pl-3'} {wide ? 'grid items-center gap-x-4' : 'flex items-center gap-3'}"
						style:grid-template-columns={wide ? listColumns(listArt) : undefined}
					>
						{#if listArt}
							<GroupArt {group} {kind} class="h-full shrink-0" />
						{/if}
						{#if wide}
							<span role="cell" class="wide truncate text-[13px] {none ? 'text-muted' : ''}">{group.name}</span>
							<span role="cell" class="label text-right text-[11px] text-muted">{group.albums.length.toLocaleString()}</span>
							<span role="cell" class="label text-right text-[11px] text-muted">{stat(group, 'tracks').toLocaleString()}</span>
							<span role="cell" class="label text-right text-[11px] text-muted">{span(stat(group, 'length'))}</span>
						{:else}
							<span role="cell" class="min-w-0 flex-1">
								<span class="wide block truncate text-[13px] leading-[18px] {none ? 'text-muted' : ''}">{group.name}</span>
								<span class="block truncate text-xs leading-4 text-muted">
									{count(group.albums.length, 'album')} · {count(stat(group, 'tracks'), 'song')}
								</span>
							</span>
							<span role="cell" class="label shrink-0 text-[11px] text-muted">{span(stat(group, 'length'))}</span>
						{/if}
					</a>
					<div role="cell" class="flex shrink-0">
						<Menu items={() => items(group)} header={header(group)} />
					</div>
				</div>
			{/each}
		</div>
	{:else}
		<ul class="relative" style:height="{Math.max(0, Math.ceil(shown.length / cols) * rowHeight)}px" {@attach rows.attach}>
			{#each visible as group, i (group.key)}
				{@const index = rows.first * cols + i}
				<li
					class="absolute top-0 left-0 border-b border-line {index % cols < cols - 1 ? 'rule-r' : ''}"
					style:width="{tile}px"
					style:height="{rowHeight}px"
					style:transform="translate({(index % cols) * tile}px, {Math.floor(index / cols) * rowHeight}px)"
					{@attach contextMenu(() => items(group), () => header(group))}
				>
					<a href={href(group)} class="block">
						<GroupArt {group} {kind} size={TILE} />
						<div class="px-2 pt-2 {kind === 'artist' ? '' : 'px-2.5'}">
							<div class="wide line-clamp-2 leading-[15px] [overflow-wrap:anywhere] {kind === 'artist' ? 'text-[11.5px]' : 'text-[12.5px]'} {group.key === NO_GENRE ? 'text-muted' : ''}">{group.name}</div>
							<div class="label mt-1 truncate text-[10px] leading-3 text-muted">{count(group.albums.length, 'album')}</div>
						</div>
					</a>
				</li>
			{/each}
		</ul>
	{/if}
</div>
