<script lang="ts">
	import { Search, X } from '@lucide/svelte';
	import { flushSync } from 'svelte';
	import { page } from '$app/state';
	import { search } from '../search.svelte.ts';

	// Phones: a search button beside the now-playing bar. Tapping it opens a
	// field in its place, focused at once so the keyboard comes up.
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
		class="glass flex h-16 min-w-0 flex-1 items-center gap-1 rounded-2xl pr-1.5 pl-4 shadow-float ring-1 ring-line"
		onsubmit={(e) => {
			e.preventDefault();
			// Search on the keyboard: put it away and show the results.
			input?.blur();
		}}
	>
		<Search class="size-5 shrink-0 text-muted" />
		<input
			bind:this={input}
			bind:value={search.query}
			type="search"
			enterkeyhint="search"
			{placeholder}
			autocomplete="off"
			autocapitalize="off"
			spellcheck="false"
			class="h-full min-w-0 flex-1 bg-transparent pl-1.5 text-base outline-none placeholder:text-faint [&::-webkit-search-cancel-button]:hidden"
			oninput={() => search.showResults(true)}
			{@attach (node) => node.focus()}
		/>
		{#if search.query}
			<button
				type="button"
				class="grid size-9 shrink-0 place-items-center rounded-full text-muted"
				aria-label="Clear search"
				onclick={() => {
					search.clear();
					input?.focus();
				}}
			>
				<X class="size-4" />
			</button>
		{/if}
		<button type="button" class="h-10 shrink-0 rounded-full px-3 text-[15px] font-medium text-accent" onclick={close}>Cancel</button>
	</form>
{:else}
	<button
		type="button"
		class="glass press grid size-16 shrink-0 place-items-center rounded-2xl text-fg shadow-float ring-1 ring-line"
		aria-label="Search"
		onclick={open}
	>
		<Search class="size-6" />
	</button>
{/if}
