<script lang="ts">
	// A request with a few words for someone: shown big on Now Playing when it comes on.
	import { Phone } from '@lucide/svelte';
	import { line } from '../party.svelte.ts';
	import Sheet from './Sheet.svelte';

	let note = $state('');
	let sending = $state(false);
	let field = $state<HTMLInputElement>();

	const close = () => {
		line.dedication = null;
		note = '';
	};

	async function send() {
		const dedication = line.dedication;
		if (!dedication) return;
		sending = true;
		try {
			await dedication.send(note.trim());
			close();
		} finally {
			sending = false;
		}
	}

	$effect(() => {
		if (line.dedication) field?.focus();
	});
</script>

{#if line.dedication}
	<Sheet onclose={close} label="Request with a dedication">
		<div class="border-b border-line px-3 pt-4 pb-3">
			<p class="label text-[11px] text-muted">Request with a dedication</p>
			<h2 class="wide mt-1 truncate text-[20px] leading-tight">{line.dedication.title}</h2>
			{#if line.dedication.subtitle}<p class="truncate text-sm text-muted">{line.dedication.subtitle}</p>{/if}
		</div>
		<form
			onsubmit={(e) => {
				e.preventDefault();
				void send();
			}}
		>
			<label class="block border-b border-line px-3 py-2">
				<span class="label flex justify-between text-[10px] text-muted"><span>Dedication</span><span>{note.length}/80</span></span>
				<input bind:this={field} class="w-full bg-transparent font-medium focus:outline-none" maxlength="80" autocomplete="off" placeholder="for Marcin, happy birthday" bind:value={note} />
			</label>
			<button type="submit" class="press wide flex h-14 w-full items-center justify-center gap-2.5 bg-accent text-[15px] text-accent-fg uppercase disabled:bg-surface-2 disabled:text-faint" disabled={sending}>
				<Phone class="size-4" strokeWidth={2.5} /> Request
			</button>
		</form>
		<button type="button" class="press label h-11 w-full text-[11px] text-muted" onclick={close}>Cancel</button>
	</Sheet>
{/if}
