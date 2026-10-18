<script lang="ts">
	import { ArrowDown, ArrowUp, ListEnd, Play, Search, X } from '@lucide/svelte';
	import { queue } from '../api.ts';
	import { NO_GENRE, type Group } from '../browse.ts';
	import { fold } from '../format.ts';
	import { library } from '../library.svelte.ts';
	import { contextMenu, type MenuItem } from '../menu.svelte.ts';
	import GroupArt from './GroupArt.svelte';
	import LibraryTabs from './LibraryTabs.svelte';

	// The Artists and Genres pages: search, sort and a grid of cards.
	let { kind, groups, title }: { kind: 'artist' | 'genre'; groups: Group[]; title: string } = $props();

	type SortKey = 'name' | 'albums';
	const storageKey = $derived(`${kind}Sort`);

	function load(): { key: SortKey; desc: boolean } {
		try {
			const v = JSON.parse(localStorage.getItem(`${kind}Sort`) ?? 'null');
			if ((v?.key === 'name' || v?.key === 'albums') && typeof v.desc === 'boolean') return v;
		} catch {
			/* storage blocked */
		}
		return { key: 'name', desc: false };
	}

	let query = $state('');
	let sort = $state(load());

	$effect(() => {
		try {
			localStorage.setItem(storageKey, JSON.stringify(sort));
		} catch {
			/* storage blocked */
		}
	});

	function sortBy(key: SortKey) {
		sort = sort.key === key ? { key, desc: !sort.desc } : { key, desc: key === 'albums' };
	}

	let shown = $derived.by(() => {
		const terms = fold(query).split(/\s+/).filter(Boolean);
		let list = terms.length ? groups.filter((g) => terms.every((t) => g.key.includes(t))) : groups;
		// Groups arrive A–Z; "No genre" stays last whatever the order.
		const last = list.filter((g) => g.key === NO_GENRE);
		list = list.filter((g) => g.key !== NO_GENRE);
		if (sort.key === 'albums') list = list.toSorted((a, b) => a.albums.length - b.albums.length);
		if (sort.desc) list = list.toReversed();
		return [...list, ...last];
	});

	const href = (g: Group) => `/${kind}/${encodeURIComponent(g.name)}`;
	const ids = (g: Group) => g.albums.map((a) => a.id);
	const items = (g: Group): MenuItem[] => [
		{ label: 'Play', icon: Play, run: () => queue.playAlbums(ids(g)) },
		{ label: 'Add to end of queue', icon: ListEnd, run: () => queue.albumsLast(ids(g)) }
	];
	const count = (n: number) => `${n.toLocaleString()} ${n === 1 ? 'album' : 'albums'}`;
</script>

<svelte:head><title>{title} · Virgil</title></svelte:head>

<div class="pt-safe glass sticky top-0 z-20 border-b border-line lg:border-none lg:bg-bg/80">
	<div class="mx-auto max-w-[110rem] px-4 pt-4 pb-3 lg:px-8 lg:pt-8">
		<LibraryTabs />
		<div class="flex items-center justify-between gap-3">
			<h1 class="text-[28px] font-bold tracking-tight lg:text-[32px]">{title}</h1>
			{#if library.loaded}
				<span class="text-sm text-muted tabular-nums">{shown.length.toLocaleString()}</span>
			{/if}
		</div>
		<label class="relative mt-3 block lg:max-w-md">
			<Search class="pointer-events-none absolute top-1/2 left-3.5 size-[18px] -translate-y-1/2 text-faint" />
			<input
				type="search"
				bind:value={query}
				placeholder="Search {title.toLowerCase()}"
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
	</div>
</div>

<div class="mx-auto max-w-[110rem] px-4 pt-5 lg:px-8">
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
			{query ? `No ${title.toLowerCase()} match “${query}”.` : `No ${title.toLowerCase()} yet.`}
		</p>
	{:else if kind === 'artist'}
		<ul class="grid grid-cols-3 gap-x-4 gap-y-6 sm:grid-cols-4 lg:grid-cols-6 2xl:grid-cols-8">
			{#each shown as group (group.key)}
				<li {@attach contextMenu(() => items(group), () => ({ title: group.name, subtitle: count(group.albums.length) }))}>
					<a href={href(group)} class="press block text-center">
						<GroupArt {group} {kind} class="rounded-full shadow-cover ring-1 ring-line" />
						<div class="mt-2.5 truncate text-[13px] leading-5 font-semibold">{group.name}</div>
						<div class="truncate text-xs leading-5 text-muted">{count(group.albums.length)}</div>
					</a>
				</li>
			{/each}
		</ul>
	{:else}
		<ul class="grid grid-cols-2 gap-x-3.5 gap-y-6 sm:grid-cols-3 lg:grid-cols-5 2xl:grid-cols-6">
			{#each shown as group (group.key)}
				<li {@attach contextMenu(() => items(group), () => ({ title: group.name, subtitle: count(group.albums.length) }))}>
					<a href={href(group)} class="press block">
						<GroupArt {group} {kind} class="rounded-xl shadow-cover ring-1 ring-line" />
						<div class="mt-2.5 truncate text-[13px] leading-5 font-semibold {group.key === '' ? 'text-muted italic' : ''}">{group.name}</div>
						<div class="truncate text-[13px] leading-5 text-muted">{count(group.albums.length)}</div>
					</a>
				</li>
			{/each}
		</ul>
	{/if}
</div>
