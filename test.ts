/* eslint-disable */

import { JsonlDb } from '.'

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


function main() {
  const db = new JsonlDb('test.txt')
  db.open()
  console.log(db.isOpen())

  let start = Date.now()
  let calls = 0

  while (Date.now() - start < 1000) {
    const key = `benchmark.0.test${calls}`;
    const value = makeObj(calls);
    db.add(key, value);
    calls++
  }

  console.log('calls:', calls)

  db.close()

  console.log('closed')
}

main()

// for (const val of ['foo bar', 1, false, [1, 0xfffffffff, 3], /*{ a: 'b' }, null, undefined, () => {}, Symbol('foo')*/]) {
//   try {
//     console.dir(serializeTest(val))
//   } catch (e) {
//     console.error(e)
//   }
// }

// import { PassThrough, Readable } from 'stream'

// /**
//  * Checks if there's enough data in the buffer to deserialize a complete message
//  */
// function containsCompleteMessage(data?: Buffer): boolean {
//   return !!data && data.length >= 1 && data.length >= data[0]
// }

// enum MsgType {
//   Number = 0,
//   String = 1,
// }

// type Msg =
//   | {
//       type: MsgType.Number
//       value: number
//     }
//   | {
//       type: MsgType.String
//       value: string
//     }

// async function* decode(strm: Readable): AsyncIterableIterator<Msg> {
//   let receiveBuffer = Buffer.allocUnsafe(0)

//   for await (const chunk of strm) {
//     receiveBuffer = Buffer.concat([receiveBuffer, chunk])
//     while (containsCompleteMessage(receiveBuffer)) {
//       // We have at least one complete message
//       const len = receiveBuffer[0]
//       const rawMsg = receiveBuffer.slice(1, len)
//       receiveBuffer = skipBytes(receiveBuffer, len)

//       const msgType = rawMsg[0]
//       let value: any
//       switch (msgType) {
//         case MsgType.Number:
//           value = rawMsg.readInt32BE(1)
//           break
//         case MsgType.String:
//           value = rawMsg.toString('utf8', 1)
//           break
//         default:
//           throw new Error(`Unknown message type ${msgType}`)
//       }

//       yield value
//     }
//   }

//   console.log('decode done')
// }

// function encode(value: any): Buffer {
//   let buf: Buffer
//   switch (typeof value) {
//     case "number":
//       buf = Buffer.allocUnsafe(6)
//       buf[0] = 6
//       buf[1] = MsgType.Number
//       buf.writeInt32BE(value, 2)
//       break
//     case "string":
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
//   return Buffer.from(buf.slice(n))
// }

// ;(async () => {
//   const ps = new PassThrough()

//   setImmediate(() => {
//     for (let i = 1; i <= 10; i++) {
//       ps.write(encode('foobar'))
//     }
//     ps.end()
//     console.log('writing done')
//   })

//   let counter = 0
//   for await (const item of decode(ps)) {
//     if (typeof item === 'string') {
//       counter++
//     }
//     console.log(counter)
//   }
//   console.log('reading done')
//   debugger
// })()

// process.on('unhandledRejection', (r) => {
//   throw r
// })
