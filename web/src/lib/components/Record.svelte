<script lang="ts">
	/** A vinyl record that turns at 33⅓ rpm while `spinning`, held still while `paused`. */
	let { spinning = false, paused = false, class: className = '' }: { spinning?: boolean; paused?: boolean; class?: string } = $props();
</script>

<svg viewBox="0 0 32 32" class="record {className}" aria-hidden="true">
	<g class="platter" class:spinning class:paused>
		<circle class="vinyl" cx="16" cy="16" r="15.25" />
		{#each [13, 11.4, 9.8, 8.4] as r (r)}
			<circle class="groove" cx="16" cy="16" {r} />
		{/each}
		<circle class="label" cx="16" cy="16" r="5.6" />
		<!-- Print on the label, so the turning shows. -->
		<path class="print" d="M12.6 14.6a3.8 3.8 0 0 1 6.8 0" />
		<circle class="spindle" cx="16" cy="16" r="0.85" />
	</g>
	<!-- Light on the surface stays put while the record turns. -->
	<path class="sheen" d="M5.4 9.2a12.4 12.4 0 0 1 7.2-5.9" />
</svg>

<style>
	.vinyl {
		fill: #141417;
		stroke: var(--line);
		stroke-width: 0.5;
	}
	/* Black on near-black would vanish: a lighter pressing, with a rim. */
	:global(.dark) .vinyl {
		fill: #1e1e23;
		stroke: rgb(255 255 255 / 0.14);
	}
	.groove {
		fill: none;
		stroke: rgb(255 255 255 / 0.1);
		stroke-width: 0.45;
	}
	.label {
		fill: var(--accent);
	}
	.print {
		fill: none;
		stroke: rgb(255 255 255 / 0.7);
		stroke-width: 0.9;
		stroke-linecap: round;
	}
	.spindle {
		fill: var(--bg);
	}
	.sheen {
		fill: none;
		stroke: rgb(255 255 255 / 0.22);
		stroke-width: 1.4;
		stroke-linecap: round;
	}
	.platter {
		transform-origin: 16px 16px;
	}
	.spinning {
		/* 60 s / 33⅓ turns: one revolution every 1.8 s. */
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
		.spinning {
			animation: none;
		}
	}
</style>
