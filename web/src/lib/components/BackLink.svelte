<script lang="ts">
	import { ChevronLeft } from '@lucide/svelte';
	import { page } from '$app/state';
	import { goBack } from '../back.ts';

	// A detail page's way back (and what a swipe or a pull follows, lib/swipeback.ts): to where it was
	// opened from, or to its library page, `href`, called `label`.
	let { href, label, class: className = '' }: { href: string; label: string; class?: string } = $props();

	let elsewhere = $derived(!!page.state.opener && page.state.opener !== href);
</script>

<a
	{href}
	data-back
	class="press label flex items-center gap-1 pl-2 text-[11px] {className}"
	onclick={(e) => {
		if (e.metaKey || e.ctrlKey || e.shiftKey) return;
		e.preventDefault();
		goBack(href);
	}}
>
	<ChevronLeft class="size-4" />
	{elsewhere ? 'Back' : label}
</a>
