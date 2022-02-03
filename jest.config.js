module.exports = {
	testEnvironment: "node",
	// roots: ["<rootDir>/src"],
	testRegex: "test/db.test.ts",
	moduleFileExtensions: ["ts", "tsx", "js", "jsx", "json", "node"],
	setupFilesAfterEnv: ["jest-extended"],
	collectCoverage: false,
	// collectCoverageFrom: ["src/**/*.ts", "!src/**/*.test.ts"],
	coverageReporters: ["lcov", "html", "text-summary"],
	transform: {
		"^.+\\.(t|j)sx?$": ["@swc-node/jest"],
	},
};
