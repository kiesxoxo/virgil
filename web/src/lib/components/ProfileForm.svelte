<script lang="ts">
	// Who this device is: a name (with a dice key for a random handle), a call sign, a colour.
	import { Dices } from '@lucide/svelte';
	import { hub } from '../hub.svelte.ts';
	import { COLORS, handle, signFor } from '../names.ts';
	import { line } from '../party.svelte.ts';
	import Sign from './Sign.svelte';

	let { join = false, ondone }: { join?: boolean; ondone?: () => void } = $props();

	const others = $derived(hub.party.people.filter((p) => p.id !== line.me?.id));
	let name = $state(line.self?.name ?? handle());
	let sign = $state(line.self?.sign ?? '');
	let color = $state(line.self?.color ?? 0);
	/** The sign follows the name until it is typed in. */
	let ownSign = $state(false);
	let saving = $state(false);

	let shownSign = $derived(
		ownSign && sign.trim()
			? sign.toUpperCase().replace(/[^\p{L}\p{N}]/gu, '').slice(0, 3)
			: signFor(name, others.map((p) => p.sign))
	);
	let preview = $derived({ id: '', name, sign: shownSign, color, host: false });

	function roll() {
		name = handle();
		ownSign = false;
	}

	async function save() {
		saving = true;
		try {
			await line.save(name, shownSign, color, join);
			ondone?.();
		} finally {
			saving = false;
		}
	}
</script>

<form
	onsubmit={(e) => {
		e.preventDefault();
		void save();
	}}
>
	<div class="flex items-stretch border-b border-line">
		<div class="grid w-20 shrink-0 place-items-center border-r border-line py-3"><Sign person={preview} size="md" /></div>
		<label class="min-w-0 flex-1 px-3 py-2">
			<span class="label block text-[10px] text-muted">Name</span>
			<input class="w-full bg-transparent font-medium focus:outline-none" maxlength="24" autocomplete="nickname" bind:value={name} />
		</label>
		<button type="button" class="press grid w-14 shrink-0 place-items-center border-l border-line" aria-label="Another name" title="Another name" onclick={roll}>
			<Dices class="size-5" />
		</button>
	</div>
	<label class="flex items-stretch border-b border-line">
		<span class="label flex w-20 shrink-0 items-center border-r border-line px-3 text-[10px] text-muted">Call sign</span>
		<input
			class="wide min-w-0 flex-1 bg-transparent px-3 uppercase focus:outline-none"
			maxlength="3"
			placeholder={shownSign}
			value={ownSign ? sign : ''}
			oninput={(e) => {
				sign = e.currentTarget.value;
				ownSign = sign.trim() !== '';
			}}
		/>
	</label>
	<div class="grid h-12 grid-cols-8 border-b border-line" role="radiogroup" aria-label="Colour">
		{#each { length: COLORS } as _, c (c)}
			{@const taken = others.find((p) => p.color === c)}
			<button
				type="button"
				role="radio"
				aria-checked={color === c}
				aria-label="Colour {c + 1}{taken ? `, ${taken.name}'s` : ''}"
				class="wide grid place-items-center border-l border-line text-[10px] text-black first:border-l-0 {color === c ? 'outline-[3px] -outline-offset-[6px] outline-black outline-solid' : ''}"
				style:background="var(--who-{c})"
				onclick={() => (color = c)}
			>
				{taken && color !== c ? taken.sign : ''}
			</button>
		{/each}
	</div>
	<button type="submit" class="press wide h-14 w-full bg-accent text-[15px] text-accent-fg uppercase disabled:bg-surface-2 disabled:text-faint" disabled={saving || !name.trim()}>
		{join ? 'Join' : 'Save'}
	</button>
</form>
