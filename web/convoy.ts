import { Fixture, loadFixture } from './fixture.ts';
export type { Action } from './fixture.ts';
export type ConvoyCondition = 'current' | 'control';

export const ACTIONS = [
  { id: 1, label: 'Hold convoy · check strap' },
  { id: 2, label: 'Ask Mara what she saw' },
  { id: 3, label: 'Release convoy · take receipt' },
  { id: 4, label: 'Give receipt to Sela · intermediary pouch' },
  { id: 5, label: 'Visit / return to Ivo at depot' },
  { id: 6, label: 'Advance fictional time · 1 hour' },
] as const;

export class Convoy extends Fixture {
  #timings?: { routine: number; intermediary: number };

  constructor(instance: WebAssembly.Instance, condition?: ConvoyCondition) {
    if (condition !== undefined && condition !== 'current' && condition !== 'control') throw new Error('Invalid convoy condition.');
    super(instance, ACTIONS);
    if (condition === undefined) return;
    const select = instance.exports.dw_convoy_select;
    const inspect = instance.exports.dw_convoy_condition;
    for (const [name, value] of [['dw_convoy_select', select], ['dw_convoy_condition', inspect]] as const) {
      if (typeof value !== 'function' || value.length !== 1) throw new Error(`Missing or incompatible Wasm export: ${name}.`);
    }
    const read = (field: number): number => (inspect as (field: number) => number)(field);
    const metadata = () => {
      if (read(0) !== 1) throw new Error('Unsupported Wasm convoy condition version; expected 1.');
      const id = read(1);
      if (id !== 1 && id !== 2) throw new Error('Invalid Wasm convoy condition id.');
      const routine = read(2);
      const intermediary = read(3);
      for (const value of [routine, intermediary]) {
        if (!Number.isInteger(value) || value < 1 || value > 1000) throw new Error('Invalid Wasm convoy condition timing.');
      }
      for (const field of [4, 0xffff_ffff]) {
        const value = read(field);
        if (value !== -1 && value !== 0xffff_ffff) throw new Error('Invalid Wasm convoy condition bounds.');
      }
      return { id, routine, intermediary };
    };
    metadata();
    const id = condition === 'current' ? 1 : 2;
    if ((select as (id: number) => number)(id) !== 0) throw new Error('Wasm convoy condition selection failed.');
    const selected = metadata();
    if (selected.id !== id) throw new Error('Wasm convoy condition selection mismatch.');
    this.#timings = selected;
  }

  override snapshot() {
    const view = super.snapshot();
    if (this.#timings) {
      const hours = (ticks: number) => `${ticks} ${ticks === 1 ? 'hour' : 'hours'}`;
      view.choices[2]!.label = `Release convoy · take receipt · routine pouch (${hours(this.#timings.routine)})`;
      view.choices[3]!.label = `Give receipt to Sela · intermediary pouch (${hours(this.#timings.intermediary)})`;
    }
    return view;
  }
}

export function loadConvoy(url: URL | string, condition?: ConvoyCondition): Promise<Convoy> {
  return loadFixture(url, 'convoy.wasm', instance => new Convoy(instance, condition));
}
