import { JsonlDB as JsonlDBNative, JsonlDBOptions } from "./lib";
import path from "path";

export class JsonlDB<V> implements Map<string, V> {
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

	public open(): Promise<void> {
		this._keysCache = undefined;
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

	public compress(): Promise<void> {
		return this.db.compress();
	}

	public clear(): void {
		this._keysCache?.clear();
		this.db.clear();
	}

	public delete(key: string): boolean {
		this._keysCache?.delete(key);
		return this.db.delete(key);
	}

	public set(key: string, value: V): this {
		this._keysCache?.add(key);
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
					this.db.setObject(
						key,
						value as any,
						JSON.stringify(value),
						this.deriveIndexKeys(value),
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
		return this.getKeysCached()[Symbol.iterator]();
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
		this._keysCache = undefined;
		if (typeof jsonOrFile === "string") {
			return this.db.importJsonFile(jsonOrFile);
		} else {
			// Yeah, this is weird but more performant for large objects
			return this.db.importJsonString(JSON.stringify(jsonOrFile));
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
