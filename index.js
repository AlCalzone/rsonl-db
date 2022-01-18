"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.JsonlDB = void 0;
const lib_1 = require("./lib");
/**
 * Tests whether the given variable is a real object and not an Array
 * @param it The variable to test
 */
function isObject(it) {
    // This is necessary because:
    // typeof null === 'object'
    // typeof [] === 'object'
    // [] instanceof Object === true
    return Object.prototype.toString.call(it) === "[object Object]";
}
/**
 * Tests whether the given variable is really an Array
 * @param it The variable to test
 */
function isArray(it) {
    if (Array.isArray != null)
        return Array.isArray(it);
    return Object.prototype.toString.call(it) === "[object Array]";
}
/** Checks whether a value should be stringified before passing to Rust */
function needsStringify(value) {
    if (!value || typeof value !== "object")
        return false;
    if (isObject(value)) {
        // Empirically, empty objects can be handled faster without stringifying
        for (const _key in value)
            return true;
        return false;
    }
    else if (isArray(value)) {
        // Empirically, arrays with length < 3 are faster without stringifying
        // Check for nested objects though
        return value.length < 3 && !value.some((v) => needsStringify(v));
    }
    return false;
}
class JsonlDB {
    constructor(filename, options = {}) {
        this.validateOptions(options);
        this.db = new lib_1.JsonlDB(filename, options);
    }
    validateOptions(options /*<V>*/) {
        // @ts-expect-error
        if (options.autoCompress) {
            const { sizeFactor, sizeFactorMinimumSize, intervalMs, intervalMinChanges,
            // @ts-expect-error
             } = options.autoCompress;
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
    close() {
        return this.db.close();
    }
    get isOpen() {
        return this.db.isOpen();
    }
    clear() {
        this.db.clear();
        throw new Error("Method not implemented.");
    }
    delete(key) {
        return this.db.delete(key);
    }
    // The set method is more performant for some values when we stringify them in JS code
    set(key, value) {
        if (needsStringify(value)) {
            this.db.setStringified(key, JSON.stringify(value));
        }
        else {
            this.db.set(key, value);
        }
        return this;
    }
    get(key) {
        return this.db.get(key);
    }
    has(key) {
        return this.db.has(key);
    }
    get size() {
        return this.db.size;
    }
    forEach(callback, thisArg) {
        this.db.forEach((v, k) => {
            callback.call(thisArg, v, k, this);
        });
    }
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
}
exports.JsonlDB = JsonlDB;
