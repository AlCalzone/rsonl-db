/* eslint-disable */

export class ExternalObject<T> {
	readonly "": {
		readonly "": unique symbol;
		[K: symbol]: T;
	};
}
export interface JsonlDBOptions {
	ignoreReadErrors?: boolean | undefined | null;
	throttleFS?: JsonlDBOptionsThrottleFS | undefined | null;
}
export interface JsonlDBOptionsThrottleFS {
	intervalMs: number;
	maxBufferedCommands?: number | undefined | null;
}
export class JsonlDB {
	constructor(filename: string, options?: JsonlDBOptions | undefined | null);
	open(): Promise<void>;
	close(): Promise<void>;
	isOpen(): boolean;
	set(key: string, value: any): void;
	setStringified(key: string, value: string): void;
	delete(key: string): boolean;
	has(key: string): boolean;
	get(key: string): any | undefined | null;
	clear(): void;
	get size(): number;
	forEach(callback: (value: any, key: string) => void): void;
	getKeys(): Array<string>;
}
