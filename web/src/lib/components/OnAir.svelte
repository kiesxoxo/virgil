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
	<span class="label inline-flex shrink-0 items-center gap-1.5 text-[11px] text-accent">
		<span class="size-1.5 rounded-full bg-accent"></span>LIVE
	</span>
{/snippet}

{#if show && progress !== null}
	<div class="w-full">
		<div class="flex items-center gap-3">
			{#if compact}
				{@render live()}
				<span class="label shrink-0 text-[11px] whitespace-nowrap text-muted">{hhmm(show.start)}</span>
			{/if}
			<div class="flex flex-1 items-center {compact ? 'h-6' : 'h-11'}" role="progressbar" aria-label="{show.title}, on air" aria-valuenow={Math.round(progress * 100)}>
				<div class="w-full overflow-hidden border border-accent {compact ? 'h-2.5' : 'h-3.5'}">
					<div class="h-full bg-accent" style:transform="translateX({progress * 100 - 100}%)"></div>
				</div>
			</div>
			{#if compact}<span class="label shrink-0 text-[11px] whitespace-nowrap text-muted">{hhmm(show.end)}</span>{/if}
		</div>
		{#if !compact}
			<div class="label -mt-1 flex items-center justify-between text-[11px] text-muted">
				<span>{hhmm(show.start)}</span>
				{@render live()}
				<span>{hhmm(show.end)}</span>
			</div>
		{/if}
	</div>
{:else}
	<div class="label flex w-full items-center justify-center gap-1.5 text-[11px] text-muted {compact ? 'h-6' : 'h-11'}">
		{#if station && !station.now}
			<InfinityIcon class="size-3.5" />Infinite Mixtape
		{:else}
			{@render live()}
		{/if}
	</div>
{/if}
