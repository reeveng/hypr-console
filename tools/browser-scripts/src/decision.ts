import * as Option from "effect/Option"

export const when = (condition: boolean): Option.Option<void> =>
  Option.filter(Option.some(undefined), () => condition)
