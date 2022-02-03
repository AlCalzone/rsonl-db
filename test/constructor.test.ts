import test from "ava";
import { JsonlDB } from "..";

test("throws when autoCompress.sizeFactor <= 1", (t) => {
	t.throws(
		() =>
			new JsonlDB("foo", {
				autoCompress: {
					sizeFactor: 0.9,
				},
			}),
		{ message: /sizeFactor/ },
	);
});

test("throws when autoCompress.minimumSize <= 0", (t) => {
	t.throws(
		() =>
			new JsonlDB("foo", {
				autoCompress: {
					sizeFactorMinimumSize: -1,
				},
			}),
		{ message: /sizeFactorMinimumSize/ },
	);
});

test("throws when throttleFS.intervalMs < 10", (t) => {
	t.throws(
		() =>
			new JsonlDB("foo", {
				autoCompress: {
					intervalMs: 9,
				},
			}),
		{ message: /intervalMs/ },
	);
});

test("throws when throttleFS.intervalMinChanges < 10", (t) => {
	t.throws(
		() =>
			new JsonlDB("foo", {
				autoCompress: {
					intervalMinChanges: 0,
				},
			}),
		{ message: /intervalMinChanges/ },
	);
});

test("throws when throttleFS.intervalMs < 0", (t) => {
	t.throws(
		() =>
			new JsonlDB("foo", {
				throttleFS: {
					intervalMs: -1,
				},
			}),
		{ message: /intervalMs/ },
	);
});

test("throws when throttleFS.maxBufferedCommands < 0", (t) => {
	t.throws(
		() =>
			new JsonlDB("foo", {
				throttleFS: {
					intervalMs: 0,
					maxBufferedCommands: -1,
				},
			}),
		{ message: /maxBufferedCommands/ },
	);
});
