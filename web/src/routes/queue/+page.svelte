<script lang="ts">
	import { ChevronDown, ChevronRight, Phone, Radio as RadioIcon } from '@lucide/svelte';
	import { SvelteSet } from 'svelte/reactivity';
	import Cover from '#lib/components/Cover.svelte';
	import Equalizer from '#lib/components/Equalizer.svelte';
	import Masthead from '#lib/components/Masthead.svelte';
	import Menu from '#lib/components/Menu.svelte';
	import Sign from '#lib/components/Sign.svelte';
	import { queueHeader, queueItems, requestBlockItems } from '#lib/actions.ts';
	import { api } from '#lib/api.ts';
	import { Confirm } from '#lib/confirm.svelte.ts';
	import { clock, plural, span } from '#lib/format.ts';
	import { hub } from '#lib/hub.svelte.ts';
	import { contextMenu } from '#lib/menu.svelte.ts';
	import { line } from '#lib/party.svelte.ts';
	import { blocks, type Block } from '#lib/requests.ts';
	import { isStream, radio } from '#lib/radio.svelte.ts';
	import { toasts } from '#lib/toast.svelte.ts';
	import type { QueueItem } from '#lib/types/QueueItem.ts';

	let items = $derived(hub.queue.items);
	let cur = $derived(hub.currentIndex);
	let upNext = $derived(cur >= 0 ? items.slice(cur + 1) : items);
	let played = $derived(cur > 0 ? items.slice(0, cur) : []);
	let remaining = $derived(upNext.reduce((sum, i) => sum + i.duration, 0));

	const clearing = new Confirm();
	const clear = () => api.clear().then(() => toasts.done(hub.current ? 'Queue cleared; this song keeps playing' : 'Queue cleared'));

	const actions = (item: QueueItem) => queueItems(item, items.indexOf(item), cur);
	// With call signs on, what's next reads as requests: an album is one block, opened with a tap.
	let upNextBlocks = $derived(line.signs ? blocks(upNext) : []);
	const opened = new SvelteSet<string>();
	const blockActions = (block: Block) => requestBlockItems(block.items, cur, (item) => items.indexOf(item));
	const blockHeader = (block: Block) => ({
		title: block.items[0].album,
		subtitle: block.items[0].artist,
		art: block.items[0].art
	});
	/** A song's artist and album; a station's show and channel. */
	const subtitle = (item: QueueItem) => (isStream(item.uri) ? radio.show(item).subtitle : [item.artist, item.album].filter(Boolean).join(' · '));
</script>

<svelte:head><title>Queue · Virgil</title></svelte:head>

{#snippet row(item: QueueItem, dim: boolean)}
	{@const shown = radio.show(item)}
	<li
		class="flex items-stretch border-b border-line hover:bg-surface [contain-intrinsic-size:auto_56px] [content-visibility:auto] {dim ? 'opacity-55 hover:opacity-100' : ''}"
		{@attach contextMenu(
			() => actions(item),
			() => queueHeader(item)
		)}
	>
		<!-- Guests can't jump the queue: for them a row is only to look at (and its menu). -->
		<button type="button" class="flex min-w-0 flex-1 items-center gap-3 text-left {line.host ? 'press' : 'cursor-default'}" onclick={() => line.host && api.playItem(item.id)}>
			<Cover art={shown.art} src={shown.src} class="size-14 shrink-0" />
			<span class="min-w-0 flex-1">
				<span class="wide block truncate text-[13px] leading-[18px]">{shown.title}</span>
				<span class="block truncate text-xs leading-4 text-muted">{subtitle(item)}</span>
				{#if line.signs && item.pick?.note}<span class="block truncate text-xs leading-4">“{item.pick.note}”</span>{/if}
			</span>
			{#if line.signs}<Sign person={line.person(item.pick)} />{/if}
			{#if isStream(item.uri)}
				<RadioIcon class="size-4 text-muted" aria-label="Radio" />
			{:else}
				<span class="label text-[11px] text-muted">{clock(item.duration)}</span>
			{/if}
		</button>
		<Menu items={() => actions(item)} header={queueHeader(item)} />
	</li>
{/snippet}

{#snippet albumBlock(block: Block)}
	{@const first = block.items[0]}
	{@const open = opened.has(block.key)}
	{@const length = block.items.reduce((sum, i) => sum + i.duration, 0)}
	<li
		class="flex items-stretch border-b border-line hover:bg-surface"
		{@attach contextMenu(
			() => blockActions(block),
			() => blockHeader(block)
		)}
	>
		<button
			type="button"
			class="press flex min-w-0 flex-1 items-center gap-3 text-left"
			aria-expanded={open}
			onclick={() => (open ? opened.delete(block.key) : opened.add(block.key))}
		>
			<Cover art={first.art} class="size-14 shrink-0" />
			<span class="min-w-0 flex-1">
				<span class="wide block truncate text-[13px] leading-[18px]">{first.album}</span>
				<span class="block truncate text-xs leading-4 text-muted">{first.artist} · {plural(block.items.length, 'song')} · {span(length)}</span>
				{#if first.pick?.note}<span class="block truncate text-xs leading-4">“{first.pick.note}”</span>{/if}
			</span>
			<Sign person={line.person(block.pick)} />
			{#if open}<ChevronDown class="size-4 text-muted" />{:else}<ChevronRight class="size-4 text-muted" />{/if}
		</button>
		<Menu items={() => blockActions(block)} header={blockHeader(block)} />
	</li>
	{#if open}
		{#each block.items as item (item.id)}
			{@render row(item, false)}
		{/each}
	{/if}
{/snippet}

<div class="pt-safe">
	<Masthead word="QUEUE" />

	{#if items.length === 0}
		<div class="border-t border-line px-3 pt-8">
			<h2 class="wide text-2xl">Nothing queued</h2>
			<p class="mt-2 text-sm text-muted">{line.open ? 'Request an album or a song: it plays right away.' : 'Play an album, or add songs with “Play next”.'}</p>
			<a href="/" class="press wide mt-5 flex h-14 items-center justify-center bg-accent text-[15px] text-accent-fg uppercase">Browse albums</a>
		</div>
	{:else}
		{#if line.open}
			<button type="button" class="press flex h-11 w-full items-center gap-2 border-t border-line px-3 text-left" onclick={() => (line.sheet = true)}>
				<span class="label flex shrink-0 items-center gap-1.5 text-[10.5px] text-accent"><span class="size-1.5 rounded-full bg-current"></span>Line open</span>
				<span class="flex min-w-0 flex-1 gap-1 overflow-hidden">
					{#each line.here as person (person.id)}<Sign {person} />{/each}
				</span>
				<span class="label shrink-0 text-[10.5px] text-muted">{line.here.length} here</span>
			</button>
		{/if}
		<div class="flex h-11 items-stretch border-y border-line bg-bg deck:fixed deck:inset-x-0 deck:z-20 deck:above-player deck:border-b-0">
			<p class="label flex min-w-0 flex-1 items-center px-3 text-[10.5px] whitespace-nowrap text-muted">
				{cur >= 0 ? `${upNext.length} up next` : plural(items.length, 'song')}{#if remaining > 0}&nbsp;/ {span(remaining)}{/if}
			</p>
			{#if line.host}
			<button
				type="button"
				class="grid w-11 shrink-0 place-items-center border-l border-line {line.open ? 'press-lit bg-fg text-bg' : 'press text-muted'}"
				aria-label="Request line"
				title="Request line: friends request songs from their phones"
				onclick={() => (line.sheet = true)}
			>
				<Phone class="size-4" />
			</button>
			<button
				type="button"
				class="label border-l border-line px-3 text-[10.5px] whitespace-nowrap {hub.player.consume ? 'press-lit bg-fg text-bg' : 'press text-muted'}"
				aria-pressed={hub.player.consume}
				title="Remove songs from the queue once played"
				onclick={() => api.modes({ consume: !hub.player.consume })}
			>
				Auto-remove
			</button>
			<button
				type="button"
				class="label border-l border-line px-3 text-[10.5px] whitespace-nowrap {clearing.armed ? 'press bg-accent text-accent-fg' : 'press text-muted'}"
				title={hub.current ? 'Remove everything except the song that is playing' : 'Remove everything'}
				onclick={() => clearing.press(clear)}
			>
				{clearing.armed ? 'Confirm' : 'Clear'}
			</button>
			{/if}
		</div>

		{#if hub.current}
			{@const item = hub.current}
			{@const shown = radio.show(item)}
			<h2 class="label flex h-9 items-center border-b border-line px-3 text-[11px] text-accent">Now playing</h2>
			<a href="/now" class="flex items-center gap-3 border-b border-line pr-3">
				<Cover art={shown.art} src={shown.src} class="size-20 shrink-0" />
				<span class="min-w-0 flex-1">
					<span class="wide line-clamp-2 text-[15px] leading-[18px]">{shown.title}</span>
					<span class="mt-0.5 block truncate text-xs leading-4 text-muted">{subtitle(item)}</span>
				</span>
				{#if line.signs}<Sign person={line.person(item.pick)} size="sm" />{/if}
				<Equalizer playing={hub.playing} class="text-accent" />
			</a>
		{/if}

		{#if upNext.length}
			<h2 class="label flex h-9 items-center border-b border-line px-3 text-[11px] text-muted">{cur >= 0 ? 'Up next' : 'Queue'}</h2>
			<ul>
				{#if line.signs}
					{#each upNextBlocks as block (block.key)}
						{#if block.album}
							{@render albumBlock(block)}
						{:else}
							{#each block.items as item (item.id)}
								{@render row(item, false)}
							{/each}
						{/if}
					{/each}
				{:else}
					{#each upNext as item (item.id)}
						{@render row(item, false)}
					{/each}
				{/if}
			</ul>
		{/if}

		{#if played.length}
			<h2 class="label flex h-9 items-center border-b border-line px-3 text-[11px] text-muted">Played</h2>
			<ul>
				{#each played as item (item.id)}
					{@render row(item, true)}
				{/each}
			</ul>
		{/if}
	{/if}
</div>
