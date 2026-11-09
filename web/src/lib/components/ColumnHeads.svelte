<script lang="ts" generics="K extends string">
	import { ArrowDown, ArrowUp } from '@lucide/svelte';

	// A wide list's column heads, which sort it too. `art`: the first column is the covers, with no head.
	let {
		columns,
		template,
		art,
		sort,
		onsort
	}: {
		columns: { key: K; label: string; numeric?: boolean }[];
		template: string;
		art: boolean;
		sort: { key: K; desc: boolean };
		onsort: (key: K) => void;
	} = $props();
</script>

<div class="flex h-8 items-stretch border-t border-line" role="row">
	<div class="grid min-w-0 flex-1 items-center gap-x-4 {art ? '' : 'pl-3'}" style:grid-template-columns={template}>
		{#if art}<span></span>{/if}
		{#each columns as column (column.key)}
			{@const active = sort.key === column.key}
			<button
				type="button"
				role="columnheader"
				aria-sort={active ? (sort.desc ? 'descending' : 'ascending') : 'none'}
				class="label flex items-center gap-1 text-[10.5px] {column.numeric ? 'justify-end' : ''} {active ? 'text-accent' : 'text-muted hover:text-fg'}"
				onclick={() => onsort(column.key)}
			>
				{column.label}
				{#if active}
					{#if sort.desc}<ArrowDown class="size-3" />{:else}<ArrowUp class="size-3" />{/if}
				{/if}
			</button>
		{/each}
	</div>
	<span class="w-11 shrink-0"></span>
</div>
