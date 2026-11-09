<script lang="ts">
	import { Search, X } from '@lucide/svelte';
	import { flushSync } from 'svelte';
	import { page } from '$app/state';
	import { search } from '../search.svelte.ts';

	// Phones: a search key on the now-playing bar. Tapping it opens a field in
	// the bar's place, focused at once so the keyboard comes up. The bar lifts
	// itself above the keyboard (PlayerBar), so the focus mustn't also scroll:
	// iOS would pan the whole page up until the next re-render.
	let input = $state<HTMLInputElement>();

	const placeholders: Record<string, string> = { '/artists': 'Search artists', '/genres': 'Search genres' };
	let placeholder = $derived(placeholders[page.url.pathname] ?? 'Albums, artists, songs');

	function open() {
		search.open = true;
		// iOS only raises the keyboard for a focus() made during the tap itself,
		// so draw the field (which focuses itself) now rather than next frame.
		flushSync();
	}

	function close() {
		search.clear();
		search.open = false;
	}
</script>

{#if search.open}
	<form
		role="search"
		class="flex min-w-0 flex-1 items-stretch"
		onsubmit={(e) => {
			e.preventDefault();
			// Search on the keyboard: put it away and show the results.
			input?.blur();
		}}
	>
		<Search class="ml-3 size-5 shrink-0 self-center text-muted" />
		<input
			bind:this={input}
			bind:value={search.query}
			type="search"
			enterkeyhint="search"
			{placeholder}
			autocomplete="off"
			autocapitalize="off"
			spellcheck="false"
			class="min-w-0 flex-1 bg-transparent px-2.5 text-base outline-none placeholder:text-muted [&::-webkit-search-cancel-button]:hidden"
			oninput={() => search.showResults(true)}
			{@attach (node) => node.focus({ preventScroll: true })}
		/>
		{#if search.query}
			<button
				type="button"
				class="press grid w-11 shrink-0 place-items-center border-l border-line text-muted"
				aria-label="Clear search"
				onclick={() => {
					search.clear();
					input?.focus({ preventScroll: true });
				}}
			>
				<X class="size-4" />
			</button>
		{/if}
		<button type="button" class="press wide shrink-0 border-l border-line px-3.5 text-[11px] uppercase" onclick={close}>Cancel</button>
	</form>
{:else}
	<button type="button" class="press grid w-14 shrink-0 place-items-center border-l border-line rail:hidden arc:hidden" aria-label="Search" onclick={open}>
		<Search class="size-[22px]" />
	</button>
{/if}
