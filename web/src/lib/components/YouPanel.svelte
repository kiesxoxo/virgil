<script lang="ts">
	// Settings › You: who this device is, and "this is me too" for a second phone or an iPad.
	import { hub } from '../hub.svelte.ts';
	import { line } from '../party.svelte.ts';
	import ProfileForm from './ProfileForm.svelte';
	import Sign from './Sign.svelte';

	let linking = $state(false);
	// Only a host's device can become a host's (virgild says no otherwise, even with the line closed).
	let others = $derived(hub.party.people.filter((p) => p.id !== line.me?.id && (line.self?.host || !p.host)));
</script>

{#key line.me?.id}
	<ProfileForm />
{/key}
{#if others.length}
	<button type="button" class="press flex h-11 w-full items-center justify-between border-b border-line px-3 text-left" aria-expanded={linking} onclick={() => (linking = !linking)}>
		<span class="text-sm font-medium">Also me</span>
		<span class="label text-[10px] text-muted">{linking ? 'Hide' : 'This device is someone here'}</span>
	</button>
	{#if linking}
		<ul class="divide-y divide-line border-b border-line">
			{#each others as person (person.id)}
				<li>
					<button
						type="button"
						class="press flex min-h-12 w-full items-center gap-3 px-3 text-left"
						onclick={async () => {
							await line.link(person.id);
							linking = false;
						}}
					>
						<Sign {person} size="sm" />
						<span class="min-w-0 flex-1 truncate text-sm font-medium">{person.name}</span>
						<span class="label text-[10px] text-muted">That’s me</span>
					</button>
				</li>
			{/each}
		</ul>
	{/if}
{/if}
