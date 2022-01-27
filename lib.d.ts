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
	autoCompress?: JsonlDBOptionsAutoCompress | undefined | null;
	lockfileDirectory?: string | undefined | null;
	indexPaths?: Array<string> | undefined | null;
}
export interface JsonlDBOptionsThrottleFS {
	intervalMs: number;
	maxBufferedCommands?: number | undefined | null;
}
export interface JsonlDBOptionsAutoCompress {
	sizeFactor?: number | undefined | null;
	sizeFactorMinimumSize?: number | undefined | null;
	intervalMs?: number | undefined | null;
	intervalMinChanges?: number | undefined | null;
	onClose?: boolean | undefined | null;
	onOpen?: boolean | undefined | null;
}
export class JsonlDB {
	constructor(filename: string, options?: JsonlDBOptions | undefined | null);
	open(): Promise<void>;
	halfClose(): Promise<void>;
	close(): void;
	dump(filename: string): Promise<void>;
	compress(): Promise<void>;
	isOpen(): boolean;
	setPrimitive(key: string, value: any): void;
	setObject(
		key: string,
		value: object,
		stringified: string,
		indexKeys: Array<string>,
	): void;
	delete(key: string): boolean;
	has(key: string): boolean;
	get(key: string): unknown;
	getMany(
		startKey: string,
		endKey: string,
		objFilter?: string | undefined | null,
	): unknown[];
	clear(): void;
	get size(): number;
	getKeys(): Array<string>;
	getKeysStringified(): string;
	exportJson(filename: string, pretty: boolean): Promise<void>;
	importJsonFile(filename: string): Promise<void>;
	importJsonString(json: string): void;
}
