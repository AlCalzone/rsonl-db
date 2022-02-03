import { JsonlDB as JsonlDBNative, JsonlDBOptions } from "./lib";
import path from "path";

function wrapNativeErrorSync<T extends (...args: any[]) => any>(
	executor: T,
): ReturnType<T> {
	try {
		return executor();
	} catch (e: any) {
		throw new Error(e.message);
	}
}

async function wrapNativeErrorAsync<T extends (...args: any[]) => Promise<any>>(
	executor: T,
): Promise<Awaited<ReturnType<T>>> {
	try {
		return await executor();
	} catch (e: any) {
		throw new Error(e.message);
	}
}

export class JsonlDB<V = any> implements Map<string, V> {
	private readonly db: JsonlDBNative;
	private readonly options: JsonlDBOptions;

	public constructor(filename: string, options: JsonlDBOptions /*<V>*/ = {}) {
		this.validateOptions(options);
		if (path.isAbsolute(filename)) {
			filename = path.resolve(filename);
		}
		this.options = options;
		this.db = new JsonlDBNative(filename, options);
	}

	private validateOptions(options: JsonlDBOptions /*<V>*/): void {
		if (options.autoCompress) {
			const {
				sizeFactor,
				sizeFactorMinimumSize,
				intervalMs,
				intervalMinChanges,
			} = options.autoCompress;
			if (sizeFactor != undefined && sizeFactor <= 1) {
				throw new Error("sizeFactor must be > 1");
			}
			if (
				sizeFactorMinimumSize != undefined &&
				sizeFactorMinimumSize < 0
			) {
				throw new Error("sizeFactorMinimumSize must be >= 0");
			}
			if (intervalMs != undefined && intervalMs < 10) {
				throw new Error("intervalMs must be >= 10");
			}
			if (intervalMinChanges != undefined && intervalMinChanges < 1) {
				throw new Error("intervalMinChanges must be >= 1");
			}
		}
		if (options.throttleFS) {
			const { intervalMs, maxBufferedCommands } = options.throttleFS;
			if (intervalMs < 0) {
				throw new Error("intervalMs must be >= 0");
			}
			if (maxBufferedCommands != undefined && maxBufferedCommands < 0) {
				throw new Error("maxBufferedCommands must be >= 0");
			}
		}
	}

	public async open(): Promise<void> {
		this._keysCache = undefined;
		await wrapNativeErrorAsync(() => this.db.open());
	}

	public async close(): Promise<void> {
		if (!this.isOpen) return;

		await wrapNativeErrorAsync(async () => {
			await this.db.halfClose();
			this.db.close();
		});
	}

	public get isOpen(): boolean {
		return this.db.isOpen();
	}

	public dump(filename: string): Promise<void> {
		return wrapNativeErrorAsync(() => this.db.dump(filename));
	}

	public compress(): Promise<void> {
		return wrapNativeErrorAsync(() => this.db.compress());
	}

	public clear(): void {
		this._keysCache?.clear();
		wrapNativeErrorSync(() => this.db.clear());
	}

	public delete(key: string): boolean {
		this._keysCache?.delete(key);
		return wrapNativeErrorSync(() => this.db.delete(key));
	}

	public set(key: string, value: V): this {
		this._keysCache?.add(key);
		switch (typeof value) {
			case "number":
			case "boolean":
			case "string":
				wrapNativeErrorSync(() => this.db.setPrimitive(key, value));
				break;
			case "object":
				if (value === null) {
					wrapNativeErrorSync(() => this.db.setPrimitive(key, value));
				} else {
					wrapNativeErrorSync(() =>
						this.db.setObject(
							key,
							value as any,
							JSON.stringify(value),
							this.deriveIndexKeys(value),
						),
					);
				}
				break;
			default:
				throw new Error("unsupported value type");
		}
		return this;
	}

	public get(key: string): V | undefined {
		return wrapNativeErrorSync(() => this.db.get(key) as any);
	}

	public getMany(
		startkey: string,
		endkey: string,
		objectFilter?: string,
	): V[] {
		return wrapNativeErrorSync(
			() => this.db.getMany(startkey, endkey, objectFilter) as any,
		);
	}

	public has(key: string): boolean {
		return wrapNativeErrorSync(() => this.db.has(key));
	}
	public get size(): number {
		return wrapNativeErrorSync(() => this.db.size);
	}

	public forEach(
		callback: (value: V, key: string, map: Map<string, V>) => void,
		thisArg?: any,
	): void {
		this.db.forEach((v, k) => {
			callback.call(thisArg, v, k, this);
		});
	}

	private _keysCache: Set<string> | undefined;
	private getKeysCached(): Set<string> {
		if (!this._keysCache) {
			this._keysCache = new Set(JSON.parse(this.db.getKeysStringified()));
		}
		return this._keysCache;
	}

	private deriveIndexKeys(obj: Record<string, any>): string[] {
		if (!this.options.indexPaths?.length) return [];
		return this.options.indexPaths
			.map((p) => {
				const val = pointer(obj, p);
				if (typeof val !== "string") return undefined;
				return `${p}=${val}`;
			})
			.filter((k): k is string => !!k);
	}

	public keys(): IterableIterator<string> {
		return wrapNativeErrorSync(() =>
			this.getKeysCached()[Symbol.iterator](),
		);
	}

	public entries(): IterableIterator<[string, V]> {
		const that = this;
		return (function* () {
			for (const k of that.getKeysCached()) {
				yield [k, that.get(k)!];
			}
		})();
	}

	public values(): IterableIterator<V> {
		const that = this;
		return (function* () {
			for (const k of that.getKeysCached()) {
				yield that.get(k)!;
			}
		})();
	}

	public [Symbol.iterator](): IterableIterator<[string, V]> {
		return wrapNativeErrorSync(() => this.entries());
	}
	public get [Symbol.toStringTag](): string {
		return "JsonlDB";
	}

	public async exportJson(
		filename: string,
		pretty: boolean = false,
	): Promise<void> {
		await wrapNativeErrorAsync(() => this.db.exportJson(filename, pretty));
	}

	public importJson(filename: string): Promise<void>;
	public importJson(json: Record<string, any>): void;
	public importJson(
		jsonOrFile: Record<string, any> | string,
	): void | Promise<void> {
		this._keysCache = undefined;
		if (typeof jsonOrFile === "string") {
			return wrapNativeErrorAsync(() =>
				this.db.importJsonFile(jsonOrFile),
			);
		} else {
			// Yeah, this is weird but more performant for large objects
			return wrapNativeErrorSync(() =>
				this.db.importJsonString(JSON.stringify(jsonOrFile)),
			);
		}
	}
}

export { JsonlDBOptions, JsonlDBOptionsThrottleFS } from "./lib";

// Matches the rust implementation of serde_json::Value::pointer
function pointer(object: Record<string, any>, path: string): unknown {
	if (path === "") {
		return object;
	} else if (!path.startsWith("/")) {
		return undefined;
	}
	function _pointer(obj: Record<string, any>, pathArr: string[]): unknown {
		// are we there yet? then return obj
		if (!pathArr.length) return obj;
		// go deeper
		let propName: string | number = pathArr.shift()!;
		if (/\[\d+\]/.test(propName)) {
			// this is an array index
			propName = +propName.slice(1, -1);
		}
		return _pointer(obj[propName], pathArr);
	}
	return _pointer(object, path.split("/").slice(1));
}
