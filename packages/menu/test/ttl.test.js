import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { ttlToModel } from '../lib/ttl.js';
import { MenuError } from '../src/errors.js';

const read = (n) => readFileSync(new URL('./fixtures/' + n, import.meta.url), 'utf8');
const expected = JSON.parse(read('eona-x.model.json'));
const PFX = '@prefix menu: <https://eona-x.eu/ontology/ui-toolkit-menu#> .\n@prefix ex: <https://eona-x.eu/menu/> .\n';

test('16 eona-x.ttl matches the contract model', () => {
  assert.deepStrictEqual(ttlToModel(read('eona-x.ttl')), expected);
});

test('17 menu:order variant gives the same model', () => {
  assert.deepStrictEqual(ttlToModel(read('eona-x-order.ttl')), expected);
});

test('18 two menus need {menu}', () => {
  const t = PFX + 'ex:a a menu:Menu ; menu:items ( [ menu:label "A" ; menu:href "/a" ] ) .\n' +
    'ex:b a menu:Menu ; menu:items ( [ menu:label "B" ; menu:href "/b" ] ) .';
  assert.throws(() => ttlToModel(t), MenuError);
  const m = ttlToModel(t, { menu: 'https://eona-x.eu/menu/b' });
  assert.equal(m.items[0].href, '/b');
});

test('19 unsafe href is a validation error with path', () => {
  const t = PFX + 'ex:a a menu:Menu ; menu:items ( [ menu:label "A" ; menu:href "javascript:alert(1)" ] ) .';
  try { ttlToModel(t); assert.fail('no throw'); } catch (e) {
    assert.ok(e instanceof MenuError);
    assert.equal(e.errors[0].path, 'items[0].href');
  }
});
