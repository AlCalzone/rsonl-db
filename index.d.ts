import { JsonlDBOptions } from "./lib";
export declare class JsonlDB<V> implements Map<string, V> {
    private readonly db;
    constructor(filename: string, options?: JsonlDBOptions);
    private validateOptions;
    open(): Promise<void>;
    close(): Promise<void>;
    get isOpen(): boolean;
    dump(filename: string): Promise<void>;
    private _compressPromise;
    compress(): Promise<void>;
    clear(): void;
    delete(key: string): boolean;
    set(key: string, value: V): this;
    get(key: string, objectFilter?: string): V | undefined;
    getMany(startkey: string, endkey: string, objectFilter?: string): V[];
    has(key: string): boolean;
    get size(): number;
    forEach(callback: (value: V, key: string, map: Map<string, V>) => void, thisArg?: any): void;
    private _keysCache;
    private getKeysCached;
    keys(): IterableIterator<string>;
    entries(): IterableIterator<[string, V]>;
    values(): IterableIterator<V>;
    [Symbol.iterator](): IterableIterator<[string, V]>;
    get [Symbol.toStringTag](): string;
    exportJson(filename: string, pretty?: boolean): Promise<void>;
    importJson(filename: string): Promise<void>;
    importJson(json: Record<string, any>): void;
}
export { JsonlDBOptions, JsonlDBOptionsThrottleFS } from "./lib";
//# sourceMappingURL=index.d.ts.map