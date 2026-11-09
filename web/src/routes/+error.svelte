<script lang="ts">
	import { page } from '$app/state';

	let missing = $derived(page.status === 404);
</script>

<svelte:head><title>{missing ? 'Not found' : 'Error'} · Virgil</title></svelte:head>

<div class="pt-safe px-3">
	<h1 class="wide mt-16 text-2xl">{missing ? 'Nothing here' : 'Something went wrong'}</h1>
	<p class="mt-2 text-sm text-muted">{missing ? 'Virgil has no page at this address.' : page.error?.message}</p>
	<a href="/" class="label mt-4 inline-block text-[11px] text-accent">Back to albums</a>
</div>
