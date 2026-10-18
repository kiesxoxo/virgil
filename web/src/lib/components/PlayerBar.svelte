<script lang="ts">
	import { Airplay, ListMusic, Maximize2, Pause, Play, SkipForward } from '@lucide/svelte';
	import { api } from '../api.ts';
	import { clock } from '../format.ts';
	import { hub } from '../hub.svelte.ts';
	import { keyboard } from '../keyboard.svelte.ts';
	import { search } from '../search.svelte.ts';
	import Cover from './Cover.svelte';
	import MobileSearch from './MobileSearch.svelte';
	import Scrubber from './Scrubber.svelte';
	import SignalBadge from './SignalBadge.svelte';
	import Transport from './Transport.svelte';

	let item = $derived(hub.current);
	let progress = $derived(hub.player.duration > 0 ? (hub.elapsed / hub.player.duration) * 100 : 0);
</script>

{#snippet airplayTile(cls: string)}
	<div class="grid shrink-0 place-items-center bg-info-bg text-info {cls}"><Airplay class="size-1/2" /></div>
{/snippet}

<!-- Phones and tablets: a floating mini player above the tab bar, with search
     beside it. While the keyboard is up, the row sits on top of it. -->
<div
	class="fixed inset-x-2 z-30 flex gap-2 lg:hidden"
	style:bottom={keyboard.height > 0 ? `${keyboard.height + 8}px` : 'calc(4rem + env(safe-area-inset-bottom))'}
>
	<div
		class="glass relative flex h-16 min-w-0 flex-1 items-center gap-3 overflow-hidden rounded-2xl pr-2 pl-2.5 shadow-float ring-1 ring-line"
		class:hidden={search.open}
	>
		{#if hub.external}
			{@render airplayTile('size-11 rounded-lg')}
			<div class="min-w-0 flex-1">
				<div class="truncate text-sm font-semibold">AirPlay</div>
				<div class="truncate text-xs text-muted">Another device is playing here</div>
			</div>
			<button type="button" class="press h-9 rounded-full bg-fg px-4 text-xs font-semibold text-bg" onclick={() => api.play()}>
				Play here
			</button>
		{:else}
			<a href="/now" class="flex min-w-0 flex-1 items-center gap-3">
				<Cover art={item?.art} class="size-11 shrink-0 rounded-lg" />
				<div class="min-w-0">
					<div class="truncate text-sm font-semibold">{item?.title ?? 'Not playing'}</div>
					<div class="truncate text-xs text-muted">{item ? item.artist : 'Pick an album to start'}</div>
				</div>
			</a>
			<button
				type="button"
				class="press grid size-11 place-items-center rounded-full disabled:opacity-35"
				aria-label={hub.playing ? 'Pause' : 'Play'}
				disabled={!item}
				onclick={() => hub.toggle()}
			>
				{#if hub.playing}
					<Pause class="size-6" fill="currentColor" strokeWidth={0} />
				{:else}
					<Play class="ml-0.5 size-6" fill="currentColor" strokeWidth={0} />
				{/if}
			</button>
			<button
				type="button"
				class="press grid size-11 place-items-center rounded-full disabled:opacity-35"
				aria-label="Next"
				disabled={!item}
				onclick={() => api.next()}
			>
				<SkipForward class="size-5" fill="currentColor" />
			</button>
			<div class="absolute inset-x-3 bottom-0 h-[2px] overflow-hidden rounded-full bg-fg/8">
				<div class="h-full bg-accent transition-[width] duration-300 ease-linear" style:width="{progress}%"></div>
			</div>
		{/if}
	</div>
	<MobileSearch />
</div>

<!-- Desktop: a full-width bar. -->
<div class="glass fixed inset-x-0 bottom-0 z-30 hidden h-24 grid-cols-[1fr_minmax(0,38rem)_1fr] items-center gap-6 border-t border-line px-5 lg:grid">
	<div class="flex min-w-0 items-center gap-3.5">
		{#if hub.external}
			{@render airplayTile('size-14 rounded-lg')}
			<div class="min-w-0">
				<div class="truncate text-sm font-semibold">AirPlay</div>
				<div class="truncate text-[13px] text-muted">Another device is playing here</div>
			</div>
		{:else if item}
			<a href="/now" class="shrink-0"><Cover art={item.art} class="size-14 rounded-lg shadow-cover" /></a>
			<div class="min-w-0">
				<a href="/now" class="block truncate text-sm font-semibold hover:underline">{item.title}</a>
				{#if item.albumId}
					<a href="/album/{item.albumId}" class="block truncate text-[13px] text-muted hover:text-fg">{item.artist} — {item.album}</a>
				{:else}
					<div class="truncate text-[13px] text-muted">{item.artist}</div>
				{/if}
			</div>
		{:else}
			<div class="text-sm text-muted">Not playing</div>
		{/if}
	</div>

	<div class="flex flex-col gap-1">
		{#if hub.external}
			<div class="flex justify-center">
				<button type="button" class="press h-10 rounded-full bg-fg px-5 text-sm font-semibold text-bg" onclick={() => api.play()}>
					Play from library here
				</button>
			</div>
		{:else}
			<Transport />
			<div class="flex items-center gap-3 text-[11px] font-medium text-muted tabular-nums">
				<span class="w-10 text-right">{clock(hub.elapsed)}</span>
				<div class="flex-1">
					<Scrubber value={hub.elapsed} max={hub.player.duration} disabled={!item} times={false} onseek={(s) => hub.seek(s)} />
				</div>
				<span class="w-10">{clock(hub.player.duration)}</span>
			</div>
		{/if}
	</div>

	<div class="flex items-center justify-end gap-1.5">
		{#if hub.signal.verdict !== 'idle'}
			<a href="/settings" class="mr-2"><SignalBadge signal={hub.signal} /></a>
		{/if}
		<a href="/queue" class="press grid size-9 place-items-center rounded-full text-muted hover:bg-fg/6 hover:text-fg" aria-label="Queue">
			<ListMusic class="size-[18px]" />
		</a>
		<a href="/now" class="press grid size-9 place-items-center rounded-full text-muted hover:bg-fg/6 hover:text-fg" aria-label="Now playing">
			<Maximize2 class="size-4" />
		</a>
	</div>
</div>
