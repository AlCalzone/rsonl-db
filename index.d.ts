/* eslint-disable */

export class ExternalObject<T> {
  readonly '': {
    readonly '': unique symbol
    [K: symbol]: T
  }
}
export function serializeTest(str: any): string
export class JsonlDb {
  constructor(filename: string)
  open(): void
  close(): void
  isOpen(): boolean
  add(key: string, value: any): void
}
