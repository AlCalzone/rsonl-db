import { JsonlDBOptions } from "./lib";
export declare class JsonlDB<V> implements Map<string, V> {
    private readonly db;
    constructor(filename: string, options?: JsonlDBOptions);
    private validateOptions;
    open(): Promise<void>;
    close(): Promise<void>;
    get isOpen(): boolean;
    clear(): void;
    delete(key: string): boolean;
    set(key: string, value: V): this;
    get(key: string): V | undefined;
    has(key: string): boolean;
    get size(): number;
    forEach(callback: (value: V, key: string, map: Map<string, V>) => void, thisArg?: any): void;
    keys(): IterableIterator<string>;
    entries(): IterableIterator<[string, V]>;
    values(): IterableIterator<V>;
    [Symbol.iterator](): IterableIterator<[string, V]>;
    get [Symbol.toStringTag](): string;
}
export { JsonlDBOptions, JsonlDBOptionsThrottleFS } from "./lib";
//# sourceMappingURL=index.d.ts.map