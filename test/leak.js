// eslint-disable-next-line @typescript-eslint/no-var-requires
const { wait } = require("alcalzone-shared/async");
const { JsonlDB } = require("..");

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
	const db = new JsonlDB(`test.txt`, {
		autoCompress: {
			sizeFactor: 2,
			// sizeFactorMinimumSize: 25000,
		},
		ignoreReadErrors: true,
		throttleFS: {
			intervalMs: 5000,
			maxBufferedCommands: 1000,
		},
		indexPaths: ["/type"],
	});
	// 	// const jsdb = new JsonlDB_JS('test.txt', {
	// 	//   ignoreReadErrors: true,
	// 	// })

	await db.open();

	debugger;

	// Create many copies of the same object to check if the memory leaks
	for (let i = 0; i <= 1e8; i++) {
		db.set(`test`, makeObj("a"));
	}

	db.clear();
	await db.close();


	// debugger;
}

main();
