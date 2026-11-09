<script lang="ts">
	import type { Snippet } from 'svelte';
	import { reach } from '../reach.svelte.ts';
	import LibraryTabs from './LibraryTabs.svelte';
	import SearchField from './SearchField.svelte';

	// The library's sticky controls: which library (phones) or the search field (wide screens), the
	// view keys, the sort words, and a list's column heads under them.
	let {
		placeholder,
		count = null,
		controls,
		order,
		heads
	}: { placeholder: string; count?: number | null; controls: Snippet; order: Snippet; heads?: Snippet } = $props();

	let bar: HTMLElement;
	let onCall = $derived(reach.active === 'rail' || reach.active === 'arc');

	/** On call, a touch anywhere else puts it away, unless that's the key that calls it. */
	function outside(e: PointerEvent) {
		if (!reach.panel || !(e.target instanceof Element)) return;
		if (!bar.contains(e.target) && !e.target.closest('[data-panel-key]')) reach.panel = false;
	}
</script>

<svelte:window onpointerdown={outside} />

<!-- One-handed, it's a single row on top of the player: always there on the deck, on call otherwise. -->
<div
	bind:this={bar}
	class="top-safe sticky z-20 border-y border-line bg-bg reach:fixed reach:top-auto reach:right-[var(--side-r)] reach:left-[var(--side-l)] reach:border-b-0 reach:above-player rail:flex-col {onCall && !reach.panel ? 'reach:hidden' : 'reach:flex'}"
>
	<div class="flex h-11 reach:order-2 reach:shrink-0 rail:justify-end rail:border-t rail:border-line">
		<LibraryTabs />
		<SearchField {placeholder} />
		{#if count !== null}
			<span class="label grid place-items-center border-l border-line px-3 text-[11px] text-muted max-lg:hidden">{count.toLocaleString()}</span>
		{/if}
		{@render controls()}
	</div>
	<div class="flex h-9 items-stretch gap-1 border-t border-line px-3 reach:order-1 reach:h-11 reach:min-w-0 reach:flex-1 reach:overflow-x-auto rail:flex-none reach:border-t-0 reach:px-2 reach:[scrollbar-width:none]" data-swipe>
		{@render order()}
	</div>
	{@render heads?.()}
</div>
