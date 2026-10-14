// One typed registry: a fixed set of ids, each filled at most once by the screen that owns it.

export interface Registry<M> {
  /** Fills `id`. A second registration of the same id throws: each id has one owner. */
  register<K extends keyof M & string>(id: K, entry: M[K]): void;
  get<K extends keyof M & string>(id: K): M[K] | undefined;
  /** The registered ids, in the registry's fixed order. */
  ids(): (keyof M & string)[];
}

export function createRegistry<M>(name: string, order: readonly (keyof M & string)[]): Registry<M> {
  const entries = new Map<string, unknown>();
  return {
    register(id, entry) {
      if (entries.has(id)) throw new Error(`${name}: "${id}" is registered twice`);
      entries.set(id, entry);
    },
    get: <K extends keyof M & string>(id: K) => entries.get(id) as M[K] | undefined,
    ids: () => order.filter((id) => entries.has(id)),
  };
}
