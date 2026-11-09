<script lang="ts">
	// A person's call sign: their letters, black, on a square of their colour.
	import type { Person } from '../types/Person.ts';

	let {
		person,
		size = 'xs',
		class: cls = ''
	}: { person: Person | null | undefined; size?: 'xs' | 'sm' | 'md' | 'xl'; class?: string } = $props();

	const sizes = {
		xs: 'h-[18px] min-w-[18px] px-[3px] text-[9px]',
		sm: 'h-6 min-w-6 px-1 text-[11px]',
		md: 'h-8 min-w-8 px-1.5 text-[13px]',
		xl: 'h-16 min-w-16 px-3 text-[28px]'
	};
</script>

{#if person}
	<span
		class="wide inline-grid shrink-0 place-items-center border border-line leading-none text-black {sizes[size]} {cls}"
		style:background="var(--who-{person.color % 8})"
		title={person.name}>{person.sign}</span
	>
{/if}
