/* eslint-disable @typescript-eslint/prefer-for-of */
import b from 'benny'

import { serializeTest } from '../index'

export function serialize_js(str: unknown): string {
  // if (str === undefined) return 0
  // if (str === null) return 1
  // if (typeof str === 'boolean') return 2
  // if (typeof str === 'number') return 3
  // if (typeof str === 'string') return 4
  // if (typeof str === 'symbol') return 5
  // if (typeof str === 'object') return 6
  // if (typeof str === 'function') return 7
  // if (typeof str === 'bigint') return 9
  // return 1024
  return JSON.stringify(str)
}

// /**
//  * Checks if there's enough data in the buffer to deserialize a complete message
//  */
// function containsCompleteMessage(data: Buffer, offset: number, actualLength: number): boolean {
//   return actualLength >= 1 + offset && actualLength >= data[offset]
// }

// enum MsgType {
//   Number = 0,
//   String = 1,
// }

// // type Msg =
// //   | {
// //       type: MsgType.Number
// //       value: number
// //     }
// //   | {
// //       type: MsgType.String
// //       value: string
// //     }

// async function* decode(strm: Readable): AsyncIterableIterator<any> {
//   let receiveBuffer = Buffer.allocUnsafe(32 * 1024)
//   let bufLen = 0

//   for await (const chunk of strm) {
//     ;(chunk as Buffer).copy(receiveBuffer, bufLen)
//     bufLen += chunk.length

//     let offset = 0
//     const values: any[] = []
//     while (containsCompleteMessage(receiveBuffer, offset, bufLen)) {
//       // We have at least one complete message
//       const len = receiveBuffer[offset]
//       const rawMsg = receiveBuffer.slice(1 + offset, len + offset)
//       offset += len

//       const msgType = rawMsg[0]
//       let value: any
//       switch (msgType) {
//         case MsgType.Number:
//           value = rawMsg.readInt32BE(1, true)
//           break
//         case MsgType.String:
//           value = rawMsg.toString('utf8', 1)
//           break
//         default:
//           throw new Error(`Unknown message type ${msgType}`)
//       }

//       values.push(value)
//     }

//     if (offset > 0) {
//       receiveBuffer = skipBytes(receiveBuffer, offset)
//       bufLen -= offset
//     }

//     yield* values
//   }

//   // console.log('decode done')
// }

// function encode(value: any): Buffer {
//   let buf: Buffer
//   switch (typeof value) {
//     case 'number':
//       buf = Buffer.allocUnsafe(6)
//       buf[0] = 6
//       buf[1] = MsgType.Number
//       buf.writeInt32BE(value, 2)
//       break
//     case 'string':
//       buf = Buffer.allocUnsafe(value.length + 2)
//       buf[0] = value.length + 2
//       buf[1] = MsgType.String
//       buf.write(value, 2)
//       break
//     default:
//       throw new Error(`Unknown message type ${typeof value}`)
//   }
//   return buf
// }

// export function skipBytes(buf: Buffer, n: number): Buffer {
//   return buf.copyWithin(0, n)
// }

// async function streamObjectMode() {
//   const ps = new PassThrough({
//     objectMode: true,
//   })

//   setImmediate(() => {
//     for (let i = 1; i <= 1000; i++) {
//       ps.write('Hello World')
//     }
//     ps.end()
//   })

//   let counter = 0
//   for await (const item of ps) {
//     assert(typeof item === 'string')
//     counter++
//   }
//   assert(counter === 1000)
// }

// async function stream() {
//   const ps = new PassThrough()

//   setImmediate(() => {
//     for (let i = 1; i <= 1000; i++) {
//       ps.write(encode('foobar'))
//     }
//     ps.end()
//   })

//   let counter = 0
//   for await (const item of decode(ps)) {
//     assert(typeof item === 'string')
//     counter++
//   }
//   assert(counter === 1000)
// }

// const values = [1, 1.0, 0xffff, 0xfffffffe, '', 'abc', 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', true, false, null]
const values = [0x7fffffff, 0x80000000, 0xffffffff]

async function run() {
  await b.suite(
    'rsonl-db',

    ...values.map((val) =>
      b.add(`JS: 0x${val.toString(16)}`, () => {
        serialize_js(val)
      }),
    ),
    ...values.map((val) =>
      b.add(`Rust: 0x${val.toString(16)}`, () => {
        serializeTest(val)
      }),
    ),

    b.cycle(),
    b.complete(),
    b.save({ file: 'JSvRS', format: 'chart.html' }),
  )
}

run().catch((e) => {
  console.error(e)
})
