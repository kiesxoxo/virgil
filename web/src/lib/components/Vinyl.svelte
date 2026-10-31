<script lang="ts">
	import { artUrl } from '../format.ts';

	/** A record with the album's cover as its label, turning at 33⅓ rpm while `spinning`, held still otherwise. */
	let { art, spinning = false, class: className = '' }: { art: string | null | undefined; spinning?: boolean; class?: string } = $props();

	const id = $props.id();
	let label = $derived(artUrl(art, 300));
</script>

<svg viewBox="0 0 100 100" class="vinyl {className}" aria-hidden="true">
	<defs>
		<clipPath id="{id}-label"><circle cx="50" cy="50" r="17" /></clipPath>
	</defs>
	<g class="platter" class:paused={!spinning}>
		<circle class="disc" cx="50" cy="50" r="49.5" />
		{#each [46, 43, 40, 37.5, 35, 32, 29.5, 27, 24] as r (r)}
			<circle class="groove" cx="50" cy="50" {r} />
		{/each}
		<circle class="label" cx="50" cy="50" r="17" />
		{#if label}
			<image href={label} x="33" y="33" width="34" height="34" preserveAspectRatio="xMidYMid slice" clip-path="url(#{id}-label)" />
		{/if}
		<circle class="spindle" cx="50" cy="50" r="1.4" />
	</g>
	<!-- Light on the surface stays put while the record turns. -->
	<path class="sheen" d="M14 30a42 42 0 0 1 20-17" />
	<path class="sheen" d="M86 70a42 42 0 0 1-20 17" />
</svg>

<style>
	.disc {
		fill: #121215;
		stroke: rgb(255 255 255 / 0.12);
		stroke-width: 0.4;
	}
	.groove {
		fill: none;
		stroke: rgb(255 255 255 / 0.06);
		stroke-width: 0.35;
	}
	.label {
		fill: var(--accent);
	}
	.spindle {
		fill: var(--bg);
	}
	.sheen {
		fill: none;
		stroke: rgb(255 255 255 / 0.14);
		stroke-width: 2.4;
		stroke-linecap: round;
	}
	.platter {
		transform-origin: 50px 50px;
		animation: turn 1.8s linear infinite;
	}
	.paused {
		animation-play-state: paused;
	}
	@keyframes turn {
		to {
			transform: rotate(360deg);
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.platter {
			animation: none;
		}
	}
</style>
