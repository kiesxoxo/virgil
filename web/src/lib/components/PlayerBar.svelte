<script lang="ts">
	import { Airplay, ListMusic, Maximize2, Pause, Play, SkipForward } from '@lucide/svelte';
	import { api } from '../api.ts';
	import { clock } from '../format.ts';
	import { hub } from '../hub.svelte.ts';
	import { isStream, radio } from '../radio.svelte.ts';
	import { keyboard } from '../keyboard.svelte.ts';
	import { search } from '../search.svelte.ts';
	import Cover from './Cover.svelte';
	import OnAir from './OnAir.svelte';
	import MobileSearch from './MobileSearch.svelte';
	import Scrubber from './Scrubber.svelte';
	import SignalBadge from './SignalBadge.svelte';
	import Transport from './Transport.svelte';

	let item = $derived(hub.current);
	let shown = $derived(item ? radio.show(item) : null);
	let stream = $derived(item ? isStream(item.uri) : false);
	// Radio: how far through the show on air.
	let showProgress = $derived(shown?.station ? radio.progress(shown.station) : null);
	let progress = $derived(
		showProgress !== null ? showProgress * 100 : hub.player.duration > 0 ? (hub.elapsed / hub.player.duration) * 100 : 0
	);
	// The position moves once a second, and the bar glides to it; back to the start of a track it jumps.
	let last = 0;
	let glide = $derived.by(() => {
		const forward = progress >= last;
		last = progress;
		return forward;
	});
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
			<a href="/now" class="touch-menu flex min-w-0 flex-1 items-center gap-3">
				<Cover art={shown?.art} src={shown?.src} name="now-cover" class="size-11 shrink-0 rounded-lg" />
				<div class="min-w-0">
					<div class="truncate text-sm font-semibold">{shown?.title ?? 'Not playing'}</div>
					<div class="truncate text-xs text-muted">{shown ? shown.subtitle : 'Pick an album or a station'}</div>
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
				<!-- Moved, not resized: a width animating all the time would lay out the page on every frame. -->
				<div class="h-full bg-accent {glide ? 'transition-transform duration-1000 ease-linear' : ''}" style:transform="translateX({progress - 100}%)"></div>
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
		{:else if item && shown}
			<a href="/now" class="shrink-0"><Cover art={shown.art} src={shown.src} name="now-cover" class="size-14 rounded-lg shadow-cover" /></a>
			<div class="min-w-0">
				<a href="/now" class="block truncate text-sm font-semibold hover:underline">{shown.title}</a>
				{#if item.albumId}
					<a href="/album/{item.albumId}" class="block truncate text-[13px] text-muted hover:text-fg">{item.artist} — {item.album}</a>
				{:else if shown.station}
					<a href="/radio" class="block truncate text-[13px] text-muted hover:text-fg">{shown.subtitle}</a>
				{:else}
					<div class="truncate text-[13px] text-muted">{shown.subtitle}</div>
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
			{#if stream}
				<OnAir station={shown?.station ?? null} compact />
			{:else}
			<div class="flex items-center gap-3 text-[11px] font-medium text-muted tabular-nums">
				<span class="w-10 text-right">{clock(hub.elapsed)}</span>
				<div class="flex-1">
					<Scrubber value={hub.elapsed} max={hub.player.duration} disabled={!item} times={false} onseek={(s) => hub.seek(s)} />
				</div>
				<span class="w-10">{clock(hub.player.duration)}</span>
			</div>
			{/if}
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
