import { Fixture, loadFixture } from './fixture.ts';

export const ACTIONS = [
  { id: 1, label: 'Submit donated appliance report' },
  { id: 2, label: 'Inspect the donated appliance' },
  { id: 3, label: 'Request cable repair' },
  { id: 4, label: 'Attempt collection' },
  { id: 5, label: 'Ask at intake' },
  { id: 6, label: 'Advance fictional time · 1 hour' },
] as const;

export class Repair extends Fixture {
  constructor(instance: WebAssembly.Instance) {
    super(instance, ACTIONS);
  }
}

export function loadRepair(url: URL | string): Promise<Repair> {
  return loadFixture(url, 'repair.wasm', instance => new Repair(instance));
}
