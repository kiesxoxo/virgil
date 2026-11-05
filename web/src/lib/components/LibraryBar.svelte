<script lang="ts">
	import type { Snippet } from 'svelte';
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
</script>

<div class="top-safe sticky z-20 border-y border-line bg-bg">
	<div class="flex h-11">
		<LibraryTabs />
		<SearchField {placeholder} />
		{#if count !== null}
			<span class="label grid place-items-center border-l border-line px-3 text-[11px] text-muted max-lg:hidden">{count.toLocaleString()}</span>
		{/if}
		{@render controls()}
	</div>
	<div class="flex h-9 items-stretch gap-1 border-t border-line px-3">
		{@render order()}
	</div>
	{@render heads?.()}
</div>
