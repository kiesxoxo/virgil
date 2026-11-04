<script lang="ts">
	import { Infinity as InfinityIcon } from '@lucide/svelte';
	import { hhmm } from '../format.ts';
	import { radio } from '../radio.svelte.ts';
	import type { Station } from '../types/Station.ts';

	// Where a scrubber would be while radio plays: a live channel's show, start
	// to end, with how far through it is; a mixtape's endless run.
	let { station, compact = false }: { station: Station | null; compact?: boolean } = $props();

	let show = $derived(station?.now ?? null);
	let progress = $derived(station ? radio.progress(station) : null);
</script>

{#snippet live()}
	<span class="inline-flex shrink-0 items-center gap-1 rounded-full bg-accent px-1.5 py-px text-[10px] font-bold tracking-wider text-accent-fg">
		<span class="size-1.5 rounded-full bg-white"></span>LIVE
	</span>
{/snippet}

{#if show && progress !== null}
	<div class="w-full">
		<div class="flex items-center gap-3 {compact ? 'text-[11px]' : ''}">
			{#if compact}{@render live()}{/if}
			{#if compact}<span class="shrink-0 text-right font-medium whitespace-nowrap text-muted tabular-nums">{hhmm(show.start)}</span>{/if}
			<div class="flex h-5 flex-1 items-center" role="progressbar" aria-label="{show.title}, on air" aria-valuenow={Math.round(progress * 100)}>
				<div class="h-1 w-full overflow-hidden rounded-full bg-fg/12">
					<div class="h-full rounded-full bg-accent " style:transform="translateX({progress * 100 - 100}%)"></div>
				</div>
			</div>
			{#if compact}<span class="shrink-0 font-medium whitespace-nowrap text-muted tabular-nums">{hhmm(show.end)}</span>{/if}
		</div>
		{#if !compact}
			<div class="mt-1 flex items-center justify-between text-[11px] font-medium text-muted tabular-nums">
				<span>{hhmm(show.start)}</span>
				{@render live()}
				<span>{hhmm(show.end)}</span>
			</div>
		{/if}
	</div>
{:else}
	<div class="flex h-5 w-full items-center justify-center gap-1.5 text-[11px] font-semibold tracking-wider text-muted uppercase {compact ? '' : 'mb-4'}">
		{#if station && !station.now}
			<InfinityIcon class="size-3.5" />Infinite Mixtape
		{:else}
			{@render live()}
		{/if}
	</div>
{/if}
