//@ts-check
/* eslint-disable */

// import assert from 'assert'
const { JsonlDB } = require('.')
const { isArray, isObject } = require('alcalzone-shared/typeguards')
// import assert from 'assert'

function makeObj(i) {
  return {
    type: 'state',
    common: {
      name: i.toString(),
      read: true,
      write: true,
      role: 'state',
      type: 'number',
    },
    native: {},
  }
}

function needsStringify(value) {
  if (!value || typeof value !== 'object') return false
  if (isObject(value)) {
    // Empirically, empty objects can be handled faster without stringifying
    for (const _key in value) return true
    return false
  } else if (isArray(value)) {
    // Empirically, arrays with length < 3 are faster without stringifying
    // Check for nested objects though
    // @ts-ignore
    return value.length < 3 && !value.some((v) => needsStringify(v))
  }
  return false
}

async function main() {
  const db = new JsonlDB('test.txt', {
    // ignoreReadErrors: true,
    throttleFS: {
      intervalMs: -500,
      //   maxBufferedCommands: 100000
    },
  })
  // const jsdb = new JsonlDB_JS('test.txt', {
  //   ignoreReadErrors: true,
  // })

  console.time('open RS')
  await db.open()
  console.timeEnd('open RS')

  console.log(`size: `, db.size)

  // db.forEach((v, k) => {
  //   console.log(k, v);
  // });
  // console.log("after foreach")

  let start = Date.now()
  let calls = 0

  while (Date.now() - start < 10000) {
    // for (let i = 0; i < 10; i++) {
    const key = `benchmark.0.test${calls}`
    const value = makeObj(calls)
    db.set(key, value)
    // assert.ok(db.has(key))
    // assert.deepStrictEqual(db.get(key), value)
    // db.delete(key)
    calls++
  }

  // console.time('foreach')
  // calls = 0
  // db.forEach((v, k) => {
  //   calls++
  // })
  // console.timeEnd('foreach')
  console.log('calls:', calls)

  // db.clear();
  console.time('close RS')
  await db.close()
  console.timeEnd('close RS')

  // console.time('open JS')
  // await jsdb.open()
  // console.timeEnd('open JS')
  // console.log(`size: `, jsdb.size)

  // console.time('close JS')
  // await jsdb.close()
  // console.timeEnd('close JS')
}

main()
