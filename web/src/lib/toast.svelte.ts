interface Toast {
	id: number;
	message: string;
	kind: 'done' | 'error';
	/** A key at the toast's right edge, e.g. Undo. */
	action?: { label: string; run: () => unknown };
}

let next = 0;

class Toasts {
	items = $state<Toast[]>([]);

	show(message: string, kind: Toast['kind'] = 'error', ms = 3500, action?: Toast['action']) {
		const id = next++;
		this.items = [...this.items.filter((t) => t.message !== message), { id, message, kind, action }];
		setTimeout(() => this.dismiss(id), ms);
	}

	/** A short confirmation, e.g. "Playing next". */
	done(message: string) {
		this.show(message, 'done', 2000);
	}

	/** A confirmation with a way back, which stays long enough to take it. */
	undoable(message: string, undo: () => unknown) {
		const run = () => undo();
		this.show(message, 'done', 6000, { label: 'Undo', run });
	}

	dismiss(id: number) {
		this.items = this.items.filter((t) => t.id !== id);
	}
}

export const toasts = new Toasts();
