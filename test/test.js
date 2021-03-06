//@ts-check
/* eslint-disable */

// const assert = require("assert");
// const {testPrimitive, testObject} = require("./lib");

// assert.strictEqual(testPrimitive(1), 1);
// assert.strictEqual(testPrimitive("1"), "1");
// assert.strictEqual(testPrimitive(true), true);
// assert.strictEqual(testObject({a: "b"}), {a: "b"});

// import assert from 'assert'
const assert = require("assert");
const { JsonlDB } = require("..");
// const { isArray, isObject } = require("alcalzone-shared/typeguards");
// const { wait } = require("alcalzone-shared/async");
// // import assert from 'assert'

function makeObj(i) {
	return {
		type: "state",
		common: {
			name: i.toString(),
			read: true,
			write: true,
			role: "state",
			type: "number",
		},
		native: {},
	};
}

async function main() {
	debugger;
	const db = new JsonlDB(`test.txt`, {
		autoCompress: {
			sizeFactor: 2,
			sizeFactorMinimumSize: 25000,
		},
		ignoreReadErrors: true,
		throttleFS: {
			intervalMs: 60000,
			maxBufferedCommands: 1000,
		},
		indexPaths: ["/type"],
	});
	// 	// const jsdb = new JsonlDB_JS('test.txt', {
	// 	//   ignoreReadErrors: true,
	// 	// })

	console.time("open RS");
	await db.open();
	console.timeEnd("open RS");

	console.log(`size: `, db.size);

	const obj = makeObj(1);
	db.set("test", obj);
	assert.deepStrictEqual(db.get("test"), obj);

	console.log("closing");
	await db.close();
	console.time("open again");
	await db.open();
	console.timeEnd("open again");

	assert.deepStrictEqual(db.get("test"), obj);
	assert.deepStrictEqual(db.get("test"), obj);

	// assert.equal(db.getMany("", "\u9999", "/type=state").length, 1);

	// 	db.set("foo", "bar");
	db.clear();
	// 	db.set("foo", "baz");

	let start = Date.now();
	let calls = 0;

	while (Date.now() - start < 3000) {
		// for (let i = 0; i < 10; i++) {
		const key = `benchmark.0.test${calls}`;
		const value = makeObj(calls);
		db.set(key, value);
		if (Math.random() < 0.2) {
			db.delete(key);
		}
		calls++;
	}

	console.log("calls:", calls);
	console.log(`size: `, db.size);

	console.time("dump");
	let compressPromise1 = db.compress().then(() => console.log("compress 1"));
	let compressPromise2 = db.compress().then(() => console.log("compress 2"));
	let compressPromise3 = db.compress().then(() => console.log("compress 3"));
	// let dumpPromise = db.dump("test.dump.txt");
	//   while (Date.now() - start < 10000) {
	for (let i = 0; i < 10000; i++) {
		const key = `backlog${i}`;
		const value = makeObj(i);
		db.set(key, value);
		// calls++
	}
	await Promise.all([compressPromise1, compressPromise2, compressPromise3]);
	// await dumpPromise
	// await compressPromise1

	console.timeEnd("dump");

	await db.exportJson("test.json", false);

	// 	// db.clear();
	console.time("close RS");
	await db.close();
	console.timeEnd("close RS");

	// 	process.exit(0);

	// 	// console.time('open JS')
	// 	// await jsdb.open()
	// 	// console.timeEnd('open JS')
	// 	// console.log(`size: `, jsdb.size)

	// 	// console.time('close JS')
	// 	// await jsdb.close()
	// 	// console.timeEnd('close JS')
}

main();
