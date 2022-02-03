import { JsonlDB } from "..";
import path from "path";
import anyTest from "ava";
import fs from "fs-extra";
import type { TestInterface } from "ava";
import { TestFS } from "./helper/testFs";

const test = anyTest as TestInterface<{
	testFS: TestFS;
	testFSRoot: string;
}>;

test.beforeEach(async (t) => {
	const testFS = new TestFS();
	const testFSRoot = await testFS.getRoot();

	await testFS.create({
		yes:
			// Final newline omitted on purpose
			'{"k":"key1","v":1}\n{"k":"key2","v":"2"}\n{"k":"key1"}',
		emptyLines: '\n{"k":"key1","v":1}\n\n\n{"k":"key2","v":"2"}\n\n',
		broken: `{"k":"key1","v":1}\n{"k":,"v":1}\n`,
		broken2: `{"k":"key1","v":1}\n{"k":"key2","v":}\n`,
		broken3: `{"k":"key1"\n`,
		reviver: `
{"k":"key1","v":1}
{"k":"key2","v":"2"}
{"k":"key1"}
{"k":"key1","v":true}`,
	});

	t.context = {
		testFS,
		testFSRoot,
	};
});

test.afterEach(async (t) => {
	await t.context.testFS.remove();
});

test("sets the isOpen property to true", async (t) => {
	const db = new JsonlDB(path.join(t.context.testFSRoot, "yes"));
	await db.open();
	t.true(db.isOpen);
	await db.close();
});

test("checks if the given file exists and creates it if it doesn't", async (t) => {
	const filename = path.join(t.context.testFSRoot, "no");
	const db = new JsonlDB(filename);
	await db.open();
	await db.close();
	t.true(await fs.pathExists(filename));
});

test("also creates leading directories if they don't exist", async (t) => {
	const filename = path.join(
		t.context.testFSRoot,
		"this/path/does/not/exist",
	);
	const db = new JsonlDB(filename);
	await db.open();
	await db.close();
	t.true(await fs.pathExists(filename));
});

test("also creates leading directories for the lockfiles if they don't exist", async (t) => {
	const lockfileDirectory = path.join(
		t.context.testFSRoot,
		"this/path/does/not/exist/either",
	);
	const db = new JsonlDB(path.join(t.context.testFSRoot, "lockfile"), {
		lockfileDirectory,
	});
	await db.open();
	await db.close();

	t.true(await fs.pathExists(lockfileDirectory));
});

test("reads the file if it exists", async (t) => {
	const db = new JsonlDB(path.join(t.context.testFSRoot, "yes"));
	await db.open();
	await db.close();
	t.pass();
});

test("throws if another DB has opened the DB file at the same time", async (t) => {
	const db1 = new JsonlDB(path.join(t.context.testFSRoot, "yes"));
	await db1.open();

	const db2 = new JsonlDB(path.join(t.context.testFSRoot, "yes"));
	t.throwsAsync(db2.open(), {
		message: /Lockfile is in use/i,
	});

	await db1.close();

	await db2.open();
	await db2.close();
});

test("should contain the correct data", async (t) => {
	const db = new JsonlDB(path.join(t.context.testFSRoot, "yes"));
	await db.open();

	t.is(db.size, 1);
	t.false(db.has("key1"));
	t.true(db.has("key2"));
	t.is(db.get("key2"), "2");

	t.deepEqual([...db.entries()], [["key2", "2"]]);

	await db.close();
});

test("skips empty input lines", async (t) => {
	const db = new JsonlDB(path.join(t.context.testFSRoot, "emptyLines"));
	await db.open();

	t.true(db.has("key1"));
	t.is(db.get("key1"), 1);
	t.true(db.has("key2"));
	t.is(db.get("key2"), "2");

	await db.close();
});

test("throws when the file contains invalid JSON", async (t) => {
	const db = new JsonlDB(path.join(t.context.testFSRoot, "broken"));
	try {
		await db.open();
		throw new Error("it did not throw");
	} catch (e: any) {
		t.regex(e.message, /invalid data/i);
		t.regex(e.message, /line 2/);
	}
});

test("throws when the file contains invalid JSON (part 2)", async (t) => {
	const db = new JsonlDB(path.join(t.context.testFSRoot, "broken2"));
	try {
		await db.open();
		throw new Error("it did not throw");
	} catch (e: any) {
		t.regex(e.message, /invalid data/i);
		t.regex(e.message, /line 2/);
	}
});

test("throws when the file contains invalid JSON (part 3)", async (t) => {
	const db = new JsonlDB(path.join(t.context.testFSRoot, "broken3"));
	try {
		await db.open();
		throw new Error("it did not throw");
	} catch (e: any) {
		t.regex(e.message, /invalid data/i);
		t.regex(e.message, /line 1/);
	}
});

test("does not throw when the file contains invalid JSON and `ignoreReadErrors` is true", async (t) => {
	const db = new JsonlDB(path.join(t.context.testFSRoot, "broken"), {
		ignoreReadErrors: true,
	});
	await t.notThrowsAsync(db.open());
	await db.close();
});

test("does not throw when the file contains invalid JSON and `ignoreReadErrors` is true (part 2)", async (t) => {
	const db = new JsonlDB(path.join(t.context.testFSRoot, "broken2"), {
		ignoreReadErrors: true,
	});
	await t.notThrowsAsync(db.open());
	await db.close();
});

test.todo(
	"transforms each value using the valueReviver function if any is passed" /*, async (t) => {
	const reviver = jest.fn().mockReturnValue("eeee");
	const db = new JsonlDB(path.join(t.context.testFSRoot, "reviver"), {
		reviver,
	});
	await db.open();
	expect(reviver).toBeCalledTimes(2);
	expect(reviver).toBeCalledWith("key2", "2");
	expect(reviver).toBeCalledWith("key1", true);

	db.forEach((v) => {
		t.is(v, "eeee");
	});
	await db.close();
} */,
);
