<script lang="ts">
	import { Search, X } from '@lucide/svelte';
	import { search } from '../search.svelte.ts';

	// Wide screens: the library's search field, in the controls row. Phones search from the bottom bar.
	let { placeholder }: { placeholder: string } = $props();
</script>

<label class="relative block min-w-0 flex-1 max-lg:hidden">
	<Search class="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted" />
	<input
		type="search"
		bind:value={search.query}
		{@attach search.register}
		{placeholder}
		autocomplete="off"
		spellcheck="false"
		class="size-full bg-transparent pr-10 pl-10 text-[15px] placeholder:text-muted focus:outline-none [&::-webkit-search-cancel-button]:hidden"
	/>
	{#if search.query}
		<button
			type="button"
			class="press absolute inset-y-0 right-0 grid w-11 place-items-center text-muted"
			aria-label="Clear search"
			onclick={() => search.clear()}
		>
			<X class="size-4" />
		</button>
	{/if}
</label>
