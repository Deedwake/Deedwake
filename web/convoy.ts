import { Fixture, loadFixture } from './fixture.ts';
export type { Action } from './fixture.ts';

export const ACTIONS = [
  { id: 1, label: 'Hold convoy · check strap' },
  { id: 2, label: 'Ask Mara what she saw' },
  { id: 3, label: 'Release convoy · take receipt' },
  { id: 4, label: 'Give receipt to Sela · priority pouch' },
  { id: 5, label: 'Visit / return to Ivo at depot' },
  { id: 6, label: 'Advance fictional time · 1 hour' },
] as const;

export class Convoy extends Fixture {
  constructor(instance: WebAssembly.Instance) {
    super(instance, ACTIONS);
  }
}

export function loadConvoy(url: URL | string): Promise<Convoy> {
  return loadFixture(url, 'convoy.wasm', instance => new Convoy(instance));
}
