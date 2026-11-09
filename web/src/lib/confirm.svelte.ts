/** A key for something hard to undo: the first press arms it for a moment, the second does it. */
export class Confirm {
	armed = $state(false);
	#timer: ReturnType<typeof setTimeout> | undefined;

	press(run: () => unknown) {
		clearTimeout(this.#timer);
		this.armed = !this.armed;
		if (this.armed) this.#timer = setTimeout(() => (this.armed = false), 3000);
		else run();
	}
}
