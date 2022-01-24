// /* eslint-disable @typescript-eslint/prefer-for-of */
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
		indexPaths: ["/type"],
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
	const randomize = false; //true;
	const percentageOther = 98;
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

	// await db.close();
	// await db.open();

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
		return db.getMany(opts.startkey, opts.endkey, "/type=state");
		// for (const key of db.keys()) {
		// 	if (key < opts.startkey || key > opts.endkey) continue;
		// 	db.get(key, "/type=state");
		// 	// db.get(key);
		// 	// const obj = db.get(key, "/type=state");
		// 	// if (obj) calls++;
		// }
	}

	const calls = getObjectView({
		startkey: "benchmark.0.test",
		endkey: "benchmark.0.test\u9999",
	}).length;

	console.log("calls", calls);

	process.exit(0);

	await b.suite(
		"rsonl-db",

		b.add("getObjectView", () => {
			getObjectView({
				startkey: "benchmark.0.test",
				endkey: "benchmark.0.test\u9999",
			});
		}),

		b.add("getObject", async () => {
			db.get("benchmark.0.test.1");
		}),

		b.add("getObjectNull", async () => {
			db.get("benchmark.0.foobar");
		}),

		b.add("setObject", async () => {
			db.set("test-key", makeObj(5));
		}),

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
