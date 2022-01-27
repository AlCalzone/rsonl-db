import { JsonlDB as JsonlDBNative, JsonlDBOptions } from "./lib";
import path from "path";

// /**
//  * Tests whether the given variable is a real object and not an Array
//  * @param it The variable to test
//  */
// function isObject<T = unknown>(it: T): it is T & Record<string, unknown> {
// 	// This is necessary because:
// 	// typeof null === 'object'
// 	// typeof [] === 'object'
// 	// [] instanceof Object === true
// 	return Object.prototype.toString.call(it) === "[object Object]";
// }

// /**
//  * Tests whether the given variable is really an Array
//  * @param it The variable to test
//  */
// function isArray<T = unknown>(it: T): it is T & unknown[] {
// 	if (Array.isArray != null) return Array.isArray(it);
// 	return Object.prototype.toString.call(it) === "[object Array]";
// }

// /** Checks whether a value should be stringified before passing to Rust */
// function needsStringify(value: unknown): boolean {
// 	if (!value || typeof value !== "object") return false;
// 	if (isObject(value)) {
// 		// Empirically, empty objects can be handled faster without stringifying
// 		for (const _key in value) return true;
// 		return false;
// 	} else if (isArray(value)) {
// 		// Empirically, arrays with length < 3 are faster without stringifying
// 		// Check for nested objects though
// 		return value.length < 3 && !value.some((v) => needsStringify(v));
// 	}
// 	return false;
// }

// @ts-expect-error
export class JsonlDB<V> implements Map<string, V> {
	private readonly db: JsonlDBNative;

	public constructor(filename: string, options: JsonlDBOptions /*<V>*/ = {}) {
		this.validateOptions(options);
		if (path.isAbsolute(filename)) {
			filename = path.resolve(filename);
		}
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

	public open(): Promise<void> {
		return this.db.open();
	}
	public async close(): Promise<void> {
		await this.db.halfClose();
		this.db.close();
	}

	public get isOpen(): boolean {
		return this.db.isOpen();
	}

	public dump(filename: string): Promise<void> {
		return this.db.dump(filename);
	}

	private _compressPromise: Promise<void> | undefined;
	public async compress(): Promise<void> {
		// We REALLY don't want to compress twice in parallel
		if (!this._compressPromise) {
			this._compressPromise = this.db.compress();
		}
		await this._compressPromise;
		this._compressPromise = undefined;
	}

	public clear(): void {
		this.db.clear();
	}

	public delete(key: string): boolean {
		return this.db.delete(key);
	}

	public set(key: string, value: V): this {
		switch (typeof value) {
			case "number":
			case "boolean":
			case "string":
				this.db.setPrimitive(key, value);
				break;
			case "object":
				if (value === null) {
					this.db.setPrimitive(key, value);
				} else {
					this.db.setObjectStringified(
						key,
						JSON.stringify(value),
						value as any,
					);
				}
				break;
			default:
				throw new Error("unsupported value type");
		}
		return this;
	}

	public get(key: string): V | undefined {
		return this.db.get(key) as any;
	}

	public getMany(
		startkey: string,
		endkey: string,
		objectFilter?: string,
	): V[] {
		return this.db.getMany(startkey, endkey, objectFilter) as any;
	}

	public has(key: string): boolean {
		return this.db.has(key);
	}
	public get size(): number {
		return this.db.size;
	}

	// public forEach(
	// 	callback: (value: V, key: string, map: Map<string, V>) => void,
	// 	thisArg?: any,
	// ): void {
	// 	this.db.forEach((v, k) => {
	// 		callback.call(thisArg, v, k, this);
	// 	});
	// }

	public keys(): IterableIterator<string> {
		const that = this;
		return (function* () {
			const allKeys = that.db.getKeys();
			for (const k of allKeys) yield k;
		})();
	}

	public entries(): IterableIterator<[string, V]> {
		const that = this;
		return (function* () {
			for (const k of that.keys()) {
				yield [k, that.get(k)!];
			}
		})();
	}

	public values(): IterableIterator<V> {
		const that = this;
		return (function* () {
			for (const k of that.keys()) {
				yield that.get(k)!;
			}
		})();
	}

	public [Symbol.iterator](): IterableIterator<[string, V]> {
		return this.entries();
	}
	public get [Symbol.toStringTag](): string {
		return "JsonlDB";
	}

	public async exportJson(
		filename: string,
		pretty: boolean = false,
	): Promise<void> {
		await this.db.exportJson(filename, pretty);
	}

	public importJson(filename: string): Promise<void>;
	public importJson(json: Record<string, any>): void;
	public importJson(
		jsonOrFile: Record<string, any> | string,
	): void | Promise<void> {
		if (typeof jsonOrFile === "string") {
			return this.db.importJsonFile(jsonOrFile);
		} else {
			// Yeah, this is weird but more performant for large objects
			return this.db.importJsonString(JSON.stringify(jsonOrFile));
		}
	}
}

export { JsonlDBOptions, JsonlDBOptionsThrottleFS } from "./lib";
