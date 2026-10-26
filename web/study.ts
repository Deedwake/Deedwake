import { Fixture, loadFixture } from './fixture.ts';
import type { Action } from './fixture.ts';
import { ACTIONS } from './convoy.ts';

export const STUDY_CONDITIONS = [
  { id: 1, key: 'precise-immediate', label: 'A · Precise retention / immediate delivery', retention: 1, initial: 0, routine: 0, intermediary: 0 },
  { id: 2, key: 'incomplete-immediate', label: 'B · Incomplete retention / immediate delivery', retention: 2, initial: 0, routine: 0, intermediary: 0 },
  { id: 3, key: 'precise-scheduled', label: 'C · Precise retention / scheduled delivery', retention: 1, initial: 3, routine: 3, intermediary: 1 },
  { id: 4, key: 'incomplete-scheduled', label: 'D · Incomplete retention / scheduled delivery', retention: 2, initial: 3, routine: 3, intermediary: 1 },
] as const;
export type StudyCondition = typeof STUDY_CONDITIONS[number]['key'];
type Configuration = typeof STUDY_CONDITIONS[number];

function sentinel(value: number): boolean {
  return value === -1 || value === 0xffff_ffff;
}

/** Browser adapter only; all observations, retention and decisions run in Rust. */
export class StudyConvoy extends Fixture {
  #configuration: Configuration;
  #read: (field: number) => number;

  constructor(instance: WebAssembly.Instance, condition: StudyCondition) {
    const configuration = STUDY_CONDITIONS.find(candidate => candidate.key === condition);
    if (!configuration) throw new Error('Invalid convoy study condition.');
    super(instance, ACTIONS);
    const select = instance.exports.dw_convoy_study_select;
    const inspect = instance.exports.dw_convoy_study;
    for (const [name, value] of [['dw_convoy_study_select', select], ['dw_convoy_study', inspect]] as const) {
      if (typeof value !== 'function' || value.length !== 1) throw new Error(`Missing or incompatible Wasm export: ${name}.`);
    }
    this.#configuration = configuration;
    this.#read = inspect as (field: number) => number;
    this.#validate(false);
    if ((select as (id: number) => number)(configuration.id) !== 0) throw new Error('Wasm convoy study selection failed.');
    this.#validate(true);
  }

  #validate(selected: boolean): void {
    if (this.#read(0) !== 1) throw new Error('Unsupported Wasm convoy study version; expected 1.');
    for (const field of [6, 0xffff_ffff]) {
      if (!sentinel(this.#read(field))) throw new Error('Invalid Wasm convoy study bounds.');
    }
    if (!selected) {
      if (this.#read(1) !== 0 || [2, 3, 4, 5].some(field => !sentinel(this.#read(field)))) {
        throw new Error('Invalid initial Wasm convoy study state.');
      }
      return;
    }
    const c = this.#configuration;
    const expected = [c.id, c.retention, c.initial, c.routine, c.intermediary];
    for (const [index, value] of expected.entries()) {
      if (this.#read(index + 1) !== value) throw new Error('Wasm convoy study selection metadata mismatch.');
    }
  }

  studyConfiguration(): Readonly<Configuration> {
    this.#validate(true);
    return { ...this.#configuration };
  }

  override act(action: Action): number {
    this.#validate(true);
    const result = super.act(action);
    this.#validate(true);
    return result;
  }

  override developerTrace(): string {
    this.#validate(true);
    return super.developerTrace();
  }

  override snapshot() {
    this.#validate(true);
    const view = super.snapshot();
    const duration = (ticks: number) => ticks === 0 ? 'immediate' : `${ticks} ${ticks === 1 ? 'hour' : 'hours'}`;
    view.choices[2]!.label = `Release convoy · take receipt · routine pouch (${duration(this.#configuration.routine)})`;
    view.choices[3]!.label = `Give receipt to Sela · intermediary pouch (${duration(this.#configuration.intermediary)})`;
    return view;
  }
}

export function loadStudyConvoy(url: URL | string, condition: StudyCondition): Promise<StudyConvoy> {
  return loadFixture(url, 'convoy.wasm', instance => new StudyConvoy(instance, condition));
}
