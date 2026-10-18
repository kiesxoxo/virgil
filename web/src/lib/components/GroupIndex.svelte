<script lang="ts">
	import { ArrowDown, ArrowUp, ListEnd, Play, Search, X } from '@lucide/svelte';
	import { queue } from '../api.ts';
	import { NO_GENRE, type Group } from '../browse.ts';
	import { fold, span } from '../format.ts';
	import { library } from '../library.svelte.ts';
	import { contextMenu, type MenuItem } from '../menu.svelte.ts';
	import { search } from '../search.svelte.ts';
	import { isBoolean, isView, store, stored } from '../stored.ts';
	import GroupArt from './GroupArt.svelte';
	import LibraryTabs from './LibraryTabs.svelte';
	import Menu from './Menu.svelte';
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
	const listColumns = (art: boolean) => `${art ? '2.5rem ' : ''}minmax(0,1fr) 4.5rem 4.5rem 6.5rem`;

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
	let view = $state(stored(`${prefix}View`, isView, 'grid'));
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

<div class="pt-safe glass sticky top-0 z-20 border-b border-line lg:border-none lg:bg-bg/80">
	<div class="mx-auto max-w-[110rem] px-4 pt-4 pb-3 lg:px-8 lg:pt-8">
		<div class="flex items-center justify-between gap-3">
			<h1 class="text-[28px] font-bold tracking-tight lg:text-[32px]">{title}</h1>
			<div class="flex items-center gap-2">
				{#if library.loaded}
					<span class="mr-1 text-sm text-muted tabular-nums max-sm:hidden">{shown.length.toLocaleString()}</span>
				{/if}
				<LibraryTabs />
				<ViewToggle bind:view bind:art={listArt} />
			</div>
		</div>
		<label class="relative mt-3 block max-lg:hidden lg:max-w-md">
			<Search class="pointer-events-none absolute top-1/2 left-3.5 size-[18px] -translate-y-1/2 text-faint" />
			<input
				type="search"
				bind:value={search.query}
				{@attach search.register}
				placeholder="Search {title.toLowerCase()}"
				autocomplete="off"
				spellcheck="false"
				class="h-11 w-full rounded-xl bg-surface pr-10 pl-10 text-base placeholder:text-faint focus:ring-2 focus:ring-accent/40 focus:outline-none [&::-webkit-search-cancel-button]:hidden"
			/>
			{#if search.query}
				<button
					type="button"
					class="absolute top-1/2 right-2 grid size-7 -translate-y-1/2 place-items-center rounded-full text-muted hover:text-fg"
					aria-label="Clear search"
					onclick={() => search.clear()}
				>
					<X class="size-4" />
				</button>
			{/if}
		</label>
		<div class="mt-3 flex gap-2">
			{#each [{ key: 'name', label: 'Name' }, { key: 'albums', label: 'Most albums' }] as const as chip (chip.key)}
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
		{#if view === 'list' && wide && shown.length}
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
	{#if !library.loaded}
		<div class="grid gap-x-4 gap-y-6 {kind === 'artist' ? 'grid-cols-3 sm:grid-cols-4 lg:grid-cols-6 2xl:grid-cols-8' : 'grid-cols-2 sm:grid-cols-3 lg:grid-cols-5 2xl:grid-cols-6'}">
			{#each { length: 12 } as _, i (i)}
				<div class="animate-pulse">
					<div class="aspect-square bg-surface {kind === 'artist' ? 'rounded-full' : 'rounded-xl'}"></div>
					<div class="mx-auto mt-3 h-3 w-3/4 rounded bg-surface"></div>
				</div>
			{/each}
		</div>
	{:else if shown.length === 0}
		<p class="mt-12 text-center text-sm text-muted">
			{search.query ? `No ${title.toLowerCase()} match “${search.query}”.` : `No ${title.toLowerCase()} yet.`}
		</p>
	{:else if view === 'list'}
		<div role="table" aria-label={title}>
			{#each shown as group (group.key)}
				{@const none = group.key === NO_GENRE}
				<div
					role="row"
					class="flex items-center rounded-lg hover:bg-fg/4 {listArt ? 'h-14' : wide ? 'h-10' : 'h-[52px]'}"
					{@attach contextMenu(() => items(group), () => header(group))}
				>
					<a
						href={href(group)}
						class="min-w-0 flex-1 self-stretch pl-2 {wide ? 'grid items-center gap-4' : 'flex items-center gap-3'}"
						style:grid-template-columns={wide ? listColumns(listArt) : undefined}
					>
						{#if listArt}
							<GroupArt {group} {kind} class="size-10 shrink-0 {kind === 'artist' ? 'rounded-full' : 'rounded-md'}" />
						{/if}
						{#if wide}
							<span role="cell" class="truncate text-sm font-medium {none ? 'text-muted italic' : ''}">{group.name}</span>
							<span role="cell" class="text-right text-sm text-muted tabular-nums">{group.albums.length.toLocaleString()}</span>
							<span role="cell" class="text-right text-sm text-muted tabular-nums">{stat(group, 'tracks').toLocaleString()}</span>
							<span role="cell" class="text-right text-sm text-muted tabular-nums">{span(stat(group, 'length'))}</span>
						{:else}
							<span role="cell" class="min-w-0 flex-1">
								<span class="block truncate text-[15px] leading-5 font-medium {none ? 'text-muted italic' : ''}">{group.name}</span>
								<span class="block truncate text-[13px] leading-5 text-muted">
									{count(group.albums.length, 'album')} · {count(stat(group, 'tracks'), 'song')}
								</span>
							</span>
							<span role="cell" class="shrink-0 text-xs text-muted tabular-nums">{span(stat(group, 'length'))}</span>
						{/if}
					</a>
					<div role="cell" class="flex w-10 shrink-0 justify-center">
						<Menu items={() => items(group)} header={header(group)} />
					</div>
				</div>
			{/each}
		</div>
	{:else if kind === 'artist'}
		<ul class="grid grid-cols-3 gap-x-4 gap-y-6 sm:grid-cols-4 lg:grid-cols-6 2xl:grid-cols-8">
			{#each shown as group (group.key)}
				<li {@attach contextMenu(() => items(group), () => header(group))}>
					<a href={href(group)} class="press block text-center">
						<GroupArt {group} {kind} class="rounded-full shadow-cover ring-1 ring-line" />
						<div class="mt-2.5 truncate text-[13px] leading-5 font-semibold">{group.name}</div>
						<div class="truncate text-xs leading-5 text-muted">{count(group.albums.length, 'album')}</div>
					</a>
				</li>
			{/each}
		</ul>
	{:else}
		<ul class="grid grid-cols-2 gap-x-3.5 gap-y-6 sm:grid-cols-3 lg:grid-cols-5 2xl:grid-cols-6">
			{#each shown as group (group.key)}
				<li {@attach contextMenu(() => items(group), () => header(group))}>
					<a href={href(group)} class="press block">
						<GroupArt {group} {kind} class="rounded-xl shadow-cover ring-1 ring-line" />
						<div class="mt-2.5 truncate text-[13px] leading-5 font-semibold {group.key === NO_GENRE ? 'text-muted italic' : ''}">{group.name}</div>
						<div class="truncate text-[13px] leading-5 text-muted">{count(group.albums.length, 'album')}</div>
					</a>
				</li>
			{/each}
		</ul>
	{/if}
</div>
