<script lang="ts">
	// The request line, from the Queue page.
	import { line } from '../party.svelte.ts';
	import LinePanel from './LinePanel.svelte';
	import Sheet from './Sheet.svelte';

	const close = () => (line.sheet = false);
</script>

{#if line.sheet}
	<Sheet onclose={close} label="Request line">
		<h2 class="label flex h-10 items-center border-b border-line px-3 text-[11px] text-muted">Request line</h2>
		<LinePanel onnavigate={close} />
		<button type="button" class="press wide h-14 w-full text-[13px] uppercase" onclick={close}>Done</button>
	</Sheet>
{/if}
