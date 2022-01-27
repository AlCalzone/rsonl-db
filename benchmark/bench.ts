// /* eslint-disable @typescript-eslint/prefer-for-of */
import assert from "assert";
import b from "benny";

// import { serializeTest } from '../index'
import { JsonlDB } from "../";

async function run() {
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
	});

	await db.open();
	db.clear();

	function addObj(i: number) {
		db.set(`benchmark.0.test.${i}`, {
			...makeObj(i),
			_id: `benchmark.0.test.${i}`,
		});
	}

	function makeObj(i: number) {
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

	function addOtherObj(i: number) {
		db.set(`benchmark.0.test.${i}meta`, {
			_id: `benchmark.0.test.${i}meta`,
			type: "meta",
			common: {
				name: i.toString(),
				type: "meta.folder",
			},
			native: {},
		});
	}

	const noAllObjects = 10000;
	const randomize = false;
	const percentageOther = 2;
	for (let i = 1; i <= noAllObjects; i++) {
		if (randomize) {
			if (Math.random() * 100 <= percentageOther) {
				addOtherObj(i);
			} else {
				addObj(i);
			}
		} else {
			if (i <= (percentageOther / 100) * noAllObjects) {
				addOtherObj(i);
			} else {
				addObj(i);
			}
		}
	}

	await db.close();
	await db.open();

	// assert.deepStrictEqual(db.get("benchmark.0.test.1"), {
	// 	type: "state",
	// 	common: {
	// 		name: "1",
	// 		read: true,
	// 		write: true,
	// 		role: "state",
	// 		type: "number",
	// 	},
	// 	native: {},
	// });

	// let calls = 0;
	function getObjectView(opts: { startkey: string; endkey: string }) {
		const ret = db.getMany(opts.startkey, opts.endkey, "type=state");
		// ret = ret.filter((x: any) => x.type === "state");
		assert.strictEqual(
			ret.length,
			(noAllObjects * (100 - percentageOther)) / 100,
		);
		// for (const key of db.keys()) {
		// 	if (key < opts.startkey || key > opts.endkey) continue;
		// 	db.get(key, "/type=state");
		// 	// db.get(key);
		// 	// const obj = db.get(key, "/type=state");
		// 	// if (obj) calls++;
		// }
	}

	// const calls = getObjectView({
	// 	startkey: "benchmark.0.test",
	// 	endkey: "benchmark.0.test\u9999",
	// }).length;

	// console.log("calls", calls);

	// process.exit(0);

	await b.suite(
		"rsonl-db",

		b.add("getObjectView", () => {
			getObjectView({
				startkey: "benchmark.0.test",
				endkey: "benchmark.0.test\u9999",
			});
		}),

		// b.add("getObject", async () => {
		// 	db.get("benchmark.0.test.1");
		// }),

		// b.add("getObjectNull", async () => {
		// 	db.get("benchmark.0.foobar");
		// }),

		// b.add("setObjectSmall", async () => {
		// 	db.set("test-objsml", { a: 2, b: 3 });
		// }),

		// b.add("getObjectSmall", async () => {
		// 	db.get("test-objsml");
		// }),

		// b.add("setObject", async () => {
		// 	db.set("test-obj", makeObj(Math.round(Math.random() * 10000)));
		// }),

		// b.add("getObject", async () => {
		// 	db.get("test-obj");
		// }),

		// b.add("setArray", async () => {
		// 	db.set("test-arr", ["foo", "bar", "baz"]);
		// }),

		// b.add("getArray", async () => {
		// 	db.get("test-arr");
		// }),

		// b.add("setPrimitive", async () => {
		// 	db.set("test-prim", Math.random());
		// }),

		// b.add("getPrimitive", async () => {
		// 	db.get("test-prim");
		// }),

		// b.add("getKeys", async () => {
		// 	for (const key of db.keys()) {
		// 	}
		// }),

		// b.add("spreadKeys", async () => {
		// 	[...db.keys()];
		// }),

		// b.add("getKeys -> getObj", async () => {
		// 	for (const key of db.keys()) {
		// 		db.get(key);
		// 	}
		// }),

		// b.add("spreadKeys -> getObj", async () => {
		// 	for (const key of [...db.keys()]) {
		// 		db.get(key);
		// 	}
		// }),

		b.cycle(),
		b.complete(),
	);

	await db.close();
}

run().catch((e) => {
	console.error(e);
});
