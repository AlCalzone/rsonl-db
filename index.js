"use strict";
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.JsonlDB = void 0;
const lib_1 = require("./lib");
const path_1 = __importDefault(require("path"));
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
class JsonlDB {
    constructor(filename, options = {}) {
        this.validateOptions(options);
        if (path_1.default.isAbsolute(filename)) {
            filename = path_1.default.resolve(filename);
        }
        this.db = new lib_1.JsonlDB(filename, options);
    }
    validateOptions(options /*<V>*/) {
        if (options.autoCompress) {
            const { sizeFactor, sizeFactorMinimumSize, intervalMs, intervalMinChanges, } = options.autoCompress;
            if (sizeFactor != undefined && sizeFactor <= 1) {
                throw new Error("sizeFactor must be > 1");
            }
            if (sizeFactorMinimumSize != undefined &&
                sizeFactorMinimumSize < 0) {
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
    open() {
        return this.db.open();
    }
    async close() {
        await this.db.halfClose();
        this.db.close();
    }
    get isOpen() {
        return this.db.isOpen();
    }
    dump(filename) {
        return this.db.dump(filename);
    }
    async compress() {
        // We REALLY don't want to compress twice in parallel
        if (!this._compressPromise) {
            this._compressPromise = this.db.compress();
        }
        await this._compressPromise;
        this._compressPromise = undefined;
    }
    clear() {
        this.db.clear();
    }
    delete(key) {
        return this.db.delete(key);
    }
    set(key, value) {
        switch (typeof value) {
            case "number":
            case "boolean":
            case "string":
                this.db.setPrimitive(key, value);
                break;
            case "object":
                if (value === null) {
                    this.db.setPrimitive(key, value);
                }
                else {
                    this.db.setObjectStringified(key, JSON.stringify(value), value);
                }
                break;
            default:
                throw new Error("unsupported value type");
        }
        return this;
    }
    get(key) {
        return this.db.get(key);
    }
    getMany(startkey, endkey, objectFilter) {
        return this.db.getMany(startkey, endkey, objectFilter);
    }
    has(key) {
        return this.db.has(key);
    }
    get size() {
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
    keys() {
        const that = this;
        return (function* () {
            const allKeys = that.db.getKeys();
            for (const k of allKeys)
                yield k;
        })();
    }
    entries() {
        const that = this;
        return (function* () {
            for (const k of that.keys()) {
                yield [k, that.get(k)];
            }
        })();
    }
    values() {
        const that = this;
        return (function* () {
            for (const k of that.keys()) {
                yield that.get(k);
            }
        })();
    }
    [Symbol.iterator]() {
        return this.entries();
    }
    get [Symbol.toStringTag]() {
        return "JsonlDB";
    }
    async exportJson(filename, pretty = false) {
        await this.db.exportJson(filename, pretty);
    }
    importJson(jsonOrFile) {
        if (typeof jsonOrFile === "string") {
            return this.db.importJsonFile(jsonOrFile);
        }
        else {
            // Yeah, this is weird but more performant for large objects
            return this.db.importJsonString(JSON.stringify(jsonOrFile));
        }
    }
}
exports.JsonlDB = JsonlDB;
