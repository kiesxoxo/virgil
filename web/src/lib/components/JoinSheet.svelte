<script lang="ts">
	// "Who's calling?": the first thing a phone sees when it calls in to an open request line.
	import { page } from '$app/state';
	import { line } from '../party.svelte.ts';
	import ProfileForm from './ProfileForm.svelte';
	import Sheet from './Sheet.svelte';

	const close = () => (line.joining = false);
</script>

<!-- Not on the big screen: it's for the room, not for a person. -->
{#if line.joining && page.route.id !== '/screen'}
	<Sheet onclose={close} label="Who’s calling?">
		<div class="border-b border-line px-3 pt-4 pb-3">
			<p class="label text-[11px] text-accent">The request line is open</p>
			<h2 class="wide mt-1 text-[28px] leading-none">Who’s calling?</h2>
			<p class="mt-2 text-sm text-muted">Your call sign goes on everything you request, so the room knows who to thank.</p>
		</div>
		<ProfileForm join ondone={close} />
		<button type="button" class="press label h-11 w-full text-[11px] text-muted" onclick={close}>Not now</button>
	</Sheet>
{/if}
