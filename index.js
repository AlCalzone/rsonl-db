"use strict";
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.JsonlDB = void 0;
const lib_1 = require("./lib");
const path_1 = __importDefault(require("path"));
function wrapNativeErrorSync(executor) {
    try {
        return executor();
    }
    catch (e) {
        throw new Error(e.message);
    }
}
async function wrapNativeErrorAsync(executor) {
    try {
        return await executor();
    }
    catch (e) {
        throw new Error(e.message);
    }
}
class JsonlDB {
    constructor(filename, options = {}) {
        this.validateOptions(options);
        if (path_1.default.isAbsolute(filename)) {
            filename = path_1.default.resolve(filename);
        }
        this.options = options;
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
    async open() {
        this._keysCache = undefined;
        await wrapNativeErrorAsync(() => this.db.open());
    }
    async close() {
        if (!this.isOpen)
            return;
        await wrapNativeErrorAsync(async () => {
            await this.db.halfClose();
            this.db.close();
        });
    }
    get isOpen() {
        return this.db.isOpen();
    }
    dump(filename) {
        return wrapNativeErrorAsync(() => this.db.dump(filename));
    }
    compress() {
        return wrapNativeErrorAsync(() => this.db.compress());
    }
    clear() {
        var _a;
        (_a = this._keysCache) === null || _a === void 0 ? void 0 : _a.clear();
        wrapNativeErrorSync(() => this.db.clear());
    }
    delete(key) {
        var _a;
        (_a = this._keysCache) === null || _a === void 0 ? void 0 : _a.delete(key);
        return wrapNativeErrorSync(() => this.db.delete(key));
    }
    set(key, value) {
        var _a;
        (_a = this._keysCache) === null || _a === void 0 ? void 0 : _a.add(key);
        switch (typeof value) {
            case "number":
            case "boolean":
            case "string":
                wrapNativeErrorSync(() => this.db.setPrimitive(key, value));
                break;
            case "object":
                if (value === null) {
                    wrapNativeErrorSync(() => this.db.setPrimitive(key, value));
                }
                else {
                    wrapNativeErrorSync(() => this.db.setObject(key, value, JSON.stringify(value), this.deriveIndexKeys(value)));
                }
                break;
            default:
                throw new Error("unsupported value type");
        }
        return this;
    }
    get(key) {
        return wrapNativeErrorSync(() => this.db.get(key));
    }
    getMany(startkey, endkey, objectFilter) {
        return wrapNativeErrorSync(() => this.db.getMany(startkey, endkey, objectFilter));
    }
    has(key) {
        return wrapNativeErrorSync(() => this.db.has(key));
    }
    get size() {
        return wrapNativeErrorSync(() => this.db.size);
    }
    forEach(callback, thisArg) {
        this.db.forEach((v, k) => {
            callback.call(thisArg, v, k, this);
        });
    }
    getKeysCached() {
        if (!this._keysCache) {
            this._keysCache = new Set(JSON.parse(this.db.getKeysStringified()));
        }
        return this._keysCache;
    }
    deriveIndexKeys(obj) {
        var _a;
        if (!((_a = this.options.indexPaths) === null || _a === void 0 ? void 0 : _a.length))
            return [];
        return this.options.indexPaths
            .map((p) => {
            const val = pointer(obj, p);
            if (typeof val !== "string")
                return undefined;
            return `${p}=${val}`;
        })
            .filter((k) => !!k);
    }
    keys() {
        return wrapNativeErrorSync(() => this.getKeysCached()[Symbol.iterator]());
    }
    entries() {
        const that = this;
        return (function* () {
            for (const k of that.getKeysCached()) {
                yield [k, that.get(k)];
            }
        })();
    }
    values() {
        const that = this;
        return (function* () {
            for (const k of that.getKeysCached()) {
                yield that.get(k);
            }
        })();
    }
    [Symbol.iterator]() {
        return wrapNativeErrorSync(() => this.entries());
    }
    get [Symbol.toStringTag]() {
        return "JsonlDB";
    }
    async exportJson(filename, pretty = false) {
        await wrapNativeErrorAsync(() => this.db.exportJson(filename, pretty));
    }
    importJson(jsonOrFile) {
        this._keysCache = undefined;
        if (typeof jsonOrFile === "string") {
            return wrapNativeErrorAsync(() => this.db.importJsonFile(jsonOrFile));
        }
        else {
            // Yeah, this is weird but more performant for large objects
            return wrapNativeErrorSync(() => this.db.importJsonString(JSON.stringify(jsonOrFile)));
        }
    }
}
exports.JsonlDB = JsonlDB;
// Matches the rust implementation of serde_json::Value::pointer
function pointer(object, path) {
    if (path === "") {
        return object;
    }
    else if (!path.startsWith("/")) {
        return undefined;
    }
    function _pointer(obj, pathArr) {
        // are we there yet? then return obj
        if (!pathArr.length)
            return obj;
        // go deeper
        let propName = pathArr.shift();
        if (/\[\d+\]/.test(propName)) {
            // this is an array index
            propName = +propName.slice(1, -1);
        }
        return _pointer(obj[propName], pathArr);
    }
    return _pointer(object, path.split("/").slice(1));
}
