// /* eslint-disable @typescript-eslint/prefer-for-of */
import assert from "assert";
import b from "benny";

import { testObject, testPrimitive } from "../lib";

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

async function run() {
	await b.suite(
		"perf",

		b.add("testObject: round-trip obj", () => {
			const obj = makeObj(1);
			const ret = testObject(obj);
			assert.deepStrictEqual(obj, ret);
		}),

		b.add("testPrimitive: round-trip null", () => {
			const ret = testPrimitive(null);
			assert.deepStrictEqual(null, ret);
		}),

		b.add("testPrimitive: round-trip number", () => {
			const ret = testPrimitive(1);
			assert.deepStrictEqual(1, ret);
		}),

		b.cycle(),
		b.complete(),
	);
}

run().catch((e) => {
	console.error(e);
});
