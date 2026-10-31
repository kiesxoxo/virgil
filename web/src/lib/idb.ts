// A key–value store in IndexedDB, for what is too big for localStorage (the album list).
// Like stored.ts, it fails quietly: storage can be blocked or full, and then nothing is kept.

const NAME = 'virgil';
const STORE = 'kv';

let opened: Promise<IDBDatabase> | null = null;

function open(): Promise<IDBDatabase> {
	opened ??= new Promise((resolve, reject) => {
		const req = indexedDB.open(NAME, 1);
		req.onupgradeneeded = () => req.result.createObjectStore(STORE);
		req.onsuccess = () => resolve(req.result);
		req.onerror = () => reject(req.error);
	});
	return opened;
}

async function run<T>(mode: IDBTransactionMode, use: (store: IDBObjectStore) => IDBRequest): Promise<T> {
	const db = await open();
	return new Promise((resolve, reject) => {
		const req = use(db.transaction(STORE, mode).objectStore(STORE));
		req.onsuccess = () => resolve(req.result as T);
		req.onerror = () => reject(req.error);
	});
}

/** The value saved under `key`, or undefined. */
export async function get<T>(key: string): Promise<T | undefined> {
	try {
		return await run<T | undefined>('readonly', (store) => store.get(key));
	} catch {
		return undefined;
	}
}

export async function set(key: string, value: unknown): Promise<void> {
	try {
		await run('readwrite', (store) => store.put(value, key));
	} catch {
		/* storage blocked or full */
	}
}
